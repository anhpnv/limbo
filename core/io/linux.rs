use super::Completion;
use anyhow::Result;
use std::cell::RefCell;
use std::os::unix::io::AsRawFd;
use std::rc::Rc;
use std::sync::Arc;

pub struct IO {
    ring: Rc<RefCell<io_uring::IoUring>>,
}

impl IO {
    pub fn new() -> Result<Self> {
        let ring = io_uring::IoUring::new(8)?;
        Ok(Self {
            ring: Rc::new(RefCell::new(ring)),
        })
    }

    pub fn open_file(&self, path: &str) -> Result<File> {
        let file = std::fs::File::open(path)?;
        Ok(File {
            ring: self.ring.clone(),
            file,
        })
    }

    pub fn run_once(&self) -> Result<()> {
        let mut ring = self.ring.borrow_mut();
        ring.submit_and_wait(1)?;
        let cqe = ring.completion().next().expect("completion queue is empty");
        Ok(())
    }
}

pub struct File {
    ring: Rc<RefCell<io_uring::IoUring>>,
    file: std::fs::File,
}

impl File {
    pub fn pread(&self, pos: usize, c: Arc<Completion>) -> Result<()> {
        let fd = io_uring::types::Fd(self.file.as_raw_fd());
        let read_e = {
            let mut buf = c.buf_mut();
            let len = buf.len();
            let buf = buf.as_mut_ptr();
            let ptr = Arc::into_raw(c.clone());
            io_uring::opcode::Read::new(fd, buf, len as u32 )
                .offset(pos as u64)
                .build()
                .user_data(ptr as u64)
        };
        let mut ring = self.ring.borrow_mut();
        unsafe {
            ring.submission()
                .push(&read_e)
                .expect("submission queue is full");
        }
        // TODO: move this to run_once()
        ring.submit_and_wait(1)?;
        let cqe = ring.completion().next().expect("completion queue is empty");
        let c = unsafe { Arc::from_raw(cqe.user_data() as *const Completion) };
        c.complete();
        Ok(())
    }
}