use std::cell::RefCell;
use tabled::{Table, Tabled};

#[derive(Tabled, Clone, Debug)]
pub struct Printable {
    pub(crate) task_id: String,
    pub(crate) cmd: String,
    pub(crate) cmd_status: String,
    pub(crate) cmd_output: String,
}

#[derive(Debug)]
pub(crate) struct CmdPrinter {
    data: RefCell<Vec<Printable>>,
}

impl CmdPrinter {
    pub(crate) fn new() -> Self {
        Self {
            data: RefCell::new(vec![]),
        }
    }

    pub(crate) fn add_row<F, T>(&self, task_id: String, input: T, f: F)
    where
        F: Fn(String, T) -> Printable,
    {
        let row = f(task_id, input);
        self.data.borrow_mut().push(row);
    }

    pub(crate) fn simple_print(&self) {
        for row in self.data.borrow().clone() {
            println!("--------------------------");
            println!(
                "TaskID: {}\n{}\n{}; {}\n",
                row.task_id, row.cmd, row.cmd_status, row.cmd_output
            )
        }
    }

    #[allow(dead_code)]
    pub(crate) fn table_print(self) {
        let table = Table::new(self.data.borrow().clone());
        println!("{table}\n");
    }
}
