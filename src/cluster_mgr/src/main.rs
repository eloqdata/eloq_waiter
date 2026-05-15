use anyhow::Context;
use clap::Parser;
use cluster_mgr::cli::cmd_base::CmdExecutor;
use cluster_mgr::cli::{Command, CompletionShell, SubCommand, HOME_DIR};
use owo_colors::OwoColorize;
use std::io;
use std::panic;
use std::process::exit;
use tracing::{error, info};

fn completion_script(shell: &CompletionShell) -> String {
    match shell {
        CompletionShell::Bash => bash_completion_script(),
        CompletionShell::Zsh => zsh_completion_script(),
        CompletionShell::Fish => fish_completion_script(),
    }
}

fn bash_completion_script() -> String {
    r#"_eloqctl_clusters() {
    eloqctl __complete-clusters 2>/dev/null
}

_eloqctl() {
    local cur
    COMPREPLY=()
    cur="${COMP_WORDS[COMP_CWORD]}"

    local commands="demo check proxy launch start stop restart status update update-conf apply plan remove export connect list versions upgrade monitor log-srv exec run-deps deploy install scale scalelog backup failover completion"
    if [[ ${COMP_CWORD} -eq 1 ]]; then
        COMPREPLY=( $(compgen -W "${commands}" -- "${cur}") )
        return
    fi

    case "${COMP_WORDS[1]}" in
        completion)
            if [[ ${COMP_CWORD} -eq 2 ]]; then
                COMPREPLY=( $(compgen -W "bash zsh fish" -- "${cur}") )
            fi
            return
            ;;
        monitor|log-srv)
            if [[ ${COMP_CWORD} -eq 2 ]]; then
                COMPREPLY=( $(compgen -W "start stop status" -- "${cur}") )
                return
            fi
            if [[ ${COMP_CWORD} -eq 3 ]]; then
                COMPREPLY=( $(compgen -W "$(_eloqctl_clusters)" -- "${cur}") )
                return
            fi
            ;;
        start|stop|restart|status|update|update-conf|remove|export|connect|install|scale|scalelog|backup|failover)
            if [[ ${COMP_CWORD} -eq 2 ]]; then
                COMPREPLY=( $(compgen -W "$(_eloqctl_clusters)" -- "${cur}") )
                return
            fi
            ;;
    esac
}

complete -F _eloqctl eloqctl
"#
    .to_string()
}

fn zsh_completion_script() -> String {
    r#"#compdef eloqctl

_eloqctl_clusters() {
    local -a clusters
    clusters=("${(@f)$(eloqctl __complete-clusters 2>/dev/null)}")
    _describe 'cluster' clusters
}

_eloqctl() {
    local -a commands
    commands=(
        demo
        check
        proxy
        launch
        start
        stop
        restart
        status
        update
        update-conf
        apply
        plan
        remove
        export
        connect
        list
        versions
        upgrade
        monitor
        log-srv
        exec
        run-deps
        deploy
        install
        scale
        scalelog
        backup
        failover
        completion
    )

    if (( CURRENT == 2 )); then
        _describe 'command' commands
        return
    fi

    case "$words[2]" in
        completion)
            if (( CURRENT == 3 )); then
                _values 'shell' bash zsh fish
            fi
            return
            ;;
        monitor|log-srv)
            if (( CURRENT == 3 )); then
                _values 'action' start stop status
                return
            fi
            if (( CURRENT == 4 )); then
                _eloqctl_clusters
                return
            fi
            ;;
        start|stop|restart|status|update|update-conf|remove|export|connect|install|scale|scalelog|backup|failover)
            if (( CURRENT == 3 )); then
                _eloqctl_clusters
                return
            fi
            ;;
    esac
}

compdef _eloqctl eloqctl
"#
    .to_string()
}

fn fish_completion_script() -> String {
    r#"function __eloqctl_clusters
    eloqctl __complete-clusters 2>/dev/null
end

complete -c eloqctl -f
complete -c eloqctl -n '__fish_use_subcommand' -a 'demo check proxy launch start stop restart status update update-conf apply plan remove export connect list versions upgrade monitor log-srv exec run-deps deploy install scale scalelog backup failover completion'
complete -c eloqctl -n '__fish_seen_subcommand_from completion' -a 'bash zsh fish'
complete -c eloqctl -n '__fish_seen_subcommand_from monitor log-srv; and not __fish_seen_subcommand_from start stop status' -a 'start stop status'

for cmd in start stop restart status update update-conf remove export connect install scale scalelog backup failover
    complete -c eloqctl -n "__fish_seen_subcommand_from $cmd" -a '(__eloqctl_clusters)'
end

complete -c eloqctl -n '__fish_seen_subcommand_from monitor; and __fish_seen_subcommand_from start stop status' -a '(__eloqctl_clusters)'
complete -c eloqctl -n '__fish_seen_subcommand_from log-srv; and __fish_seen_subcommand_from start stop status' -a '(__eloqctl_clusters)'
"#
    .to_string()
}

fn install_panic_hook() {
    panic::set_hook(Box::new(|panic_info| {
        let message = panic_info
            .payload()
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| {
                panic_info
                    .payload()
                    .downcast_ref::<String>()
                    .map(String::as_str)
            })
            .unwrap_or("unexpected internal error");

        if let Some(location) = panic_info.location() {
            eprintln!(
                "{}: {message}\nlocation: {}:{}\nRun again with --verbose and check the command log for details.",
                "FAIL".red(),
                location.file(),
                location.line()
            );
        } else {
            eprintln!(
                "{}: {message}\nRun again with --verbose and check the command log for details.",
                "FAIL".red()
            );
        }
    }));
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> anyhow::Result<()> {
    install_panic_hook();

    let cmd = Command::parse();
    if let Some(SubCommand::Completion { shell, output }) = &cmd.subcmd {
        let mut writer: Box<dyn io::Write> = match output {
            Some(path) => Box::new(
                std::fs::File::create(path)
                    .with_context(|| format!("failed to create completion file {path:?}"))?,
            ),
            None => Box::new(io::stdout()),
        };
        writer
            .write_all(completion_script(shell).as_bytes())
            .context("failed to write completion script")?;
        return Ok(());
    }

    let home = CmdExecutor::home_init(cmd.home).context("home dir init failed")?;
    if let Some(SubCommand::CompleteClusters) = &cmd.subcmd {
        let executor = CmdExecutor::new(home);
        let clusters = executor
            .list_cluster_names()
            .await
            .context("failed to list clusters")?;
        for cluster in clusters {
            println!("{cluster}");
        }
        return Ok(());
    }

    if let Some(sub) = cmd.subcmd {
        let log_path = home.join("logs").join(format!("last-{}.log", sub.as_ref()));
        let log_file = std::fs::File::create(&log_path).context("can't create log")?;
        tracing_subscriber::fmt()
            .with_writer(log_file)
            .with_ansi(false)
            .init();

        // Box::leak is intentional: run() requires &'static self.
        // This is a CLI tool with a single invocation, so the leak is harmless.
        let executor = Box::leak(Box::new(CmdExecutor::new(home)));
        info!("command: {:#?}", sub);
        if let Err(e) = executor.run(sub, None, cmd.quiet, cmd.verbose).await {
            error!("{}", e);
            eprintln!("{}: {e}\nlogfile: {}", "FAIL".red(), log_path.display());
            exit(1);
        }
    } else {
        println!("eloqctl is the cluster management tool of eloqdata.");
        println!("{HOME_DIR}={home:?}");
        println!("Use `eloqctl --help` to see how to use it.");
    }
    Ok(())
}
