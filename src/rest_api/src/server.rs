use crate::web_handler::{check_cmd_status, check_health, ctl_cluster, deploy_cluster};
use actix_server::{Server, ServerHandle};
use actix_web::{middleware, web, App, HttpServer};
use cluster_mgr::cli::cmd_base::CommandExecutor;
use cluster_mgr::cli::config::CONFIG_PATH_DIR;
use std::sync::{mpsc, Arc};
use std::{env, thread};
use tracing::info;

macro_rules! server_listen_addr {
    ($addr_or_port:expr, $default:expr) => {{
        if let Some(value) = $addr_or_port {
            value
        } else {
            $default
        }
    }};
}

pub struct CliMgrHttpServer {
    tx: mpsc::Sender<ServerHandle>,
    rx: mpsc::Receiver<ServerHandle>,
}

unsafe impl Send for CliMgrHttpServer {}

impl Default for CliMgrHttpServer {
    fn default() -> Self {
        Self::new()
    }
}

impl CliMgrHttpServer {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self { tx, rx }
    }

    pub async fn start(
        &'static self,
        addr: Option<String>,
        port: Option<u16>,
        config_path: String,
    ) -> anyhow::Result<()> {
        let server = CliMgrHttpServer::new_http_server(addr, port, config_path).await?;
        // let tx_send_guard = self.tx.write().await;
        let _send_rs = self.tx.send(server.handle());
        info!("Starting CliMgrHttpServer.");
        let join = thread::spawn(move || {
            actix_web::rt::System::new().block_on(async { server.await.unwrap() });
        });
        let _ = join.join();
        Ok(())
    }

    pub async fn stop(&'static self) {
        let web_handler_opt = self.rx.recv();
        if let Ok(web_handler) = web_handler_opt {
            info!("Stopping CliMgrHttpServer.");
            web_handler.stop(true).await;
        }
    }

    async fn new_http_server(
        addr: Option<String>,
        port: Option<u16>,
        config_path: String,
    ) -> anyhow::Result<Server> {
        let listen_addr = server_listen_addr!(addr, "127.0.0.1".to_string());
        let listen_port = server_listen_addr!(port, 8090);
        env::set_var(CONFIG_PATH_DIR, config_path);
        let cmd_executor = web::Data::new(Arc::new(CommandExecutor::new()));
        let server = HttpServer::new(move || {
            App::new()
                .wrap(middleware::Logger::default())
                .app_data(cmd_executor.clone())
                .service(check_health)
                .service(check_cmd_status)
                .service(deploy_cluster)
                .service(ctl_cluster)
                .service(
                    web::resource("/")
                        .route(web::get().to(|| async { "Hi man. I'm CliMgrHttpServer" })),
                )
        })
        .bind((listen_addr.as_str(), listen_port))?
        .run();
        Ok(server)
    }
}

// #[cfg(test)]
// mod tests {
//     use crate::server::CliMgrHttpServer;
//     use std::path::PathBuf;
//     use std::sync::{Arc, LazyLock, Mutex};
//     use std::thread;
//
//     static REST_SERVER: LazyLock<Mutex<CliMgrHttpServer>> =
//         LazyLock::new(|| Mutex::new(CliMgrHttpServer::new()));
//
//     #[tokio::test(flavor = "multi_thread")]
//     pub async fn test_server_start() {
//         let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
//         let config_path = manifest_dir.join("../cluster_mgr/config");
//         println!("config path = {config_path:#?}");
//
//         let srv = REST_SERVER.lock().unwrap();
//         let server = Arc::new(Box::leak(Box::new(srv)));
//         thread::spawn(async move || {
//             let srv_clone = Arc::clone(&server);
//             srv_clone
//                 .start(None, None, config_path.to_str().unwrap().to_string())
//                 .await
//                 .unwrap();
//         });
//
//         let response = reqwest::get("http://127.0.0.1:8090/check_health")
//             .await
//             .unwrap();
//         let is_success = response.status().is_success();
//         println!("response = {response:#?}, success={is_success}");
//         assert!(is_success);
//         let rsp_content = response.bytes().await.unwrap();
//         let rsp_string = String::from_utf8_lossy(rsp_content.as_ref()).to_string();
//         println!("check health response: {rsp_string}");
//         server.stop().await;
//     }
// }
