use crossbeam_channel::{Sender, Receiver, unbounded};
use std::thread;

pub struct CompilerThread {
    tx: Sender<CompileRequest>,
}

pub enum CompileRequest {
    Compile(String),
}

#[derive(Clone)]
pub enum CompileResult {
    Ok(Vec<u8>),
    Err(String),
}

impl CompilerThread {
    pub fn new(result_tx: Sender<CompileResult>) -> Self {
        let (tx, rx): (Sender<CompileRequest>, Receiver<CompileRequest>) = unbounded();
        
        thread::spawn(move || {
            for req in rx {
                match req {
                    CompileRequest::Compile(source) => {
                        match tectonic::latex_to_pdf(&source) {
                            Ok(pdf_data) => {
                                let _ = result_tx.send(CompileResult::Ok(pdf_data));
                            }
                            Err(e) => {
                                let _ = result_tx.send(CompileResult::Err(e.to_string()));
                            }
                        }
                    }
                }
            }
        });
        
        Self { tx }
    }

    pub fn compile(&self, content: String) {
        let _ = self.tx.send(CompileRequest::Compile(content));
    }
}
