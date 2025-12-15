use std::sync::{Arc, Mutex};

use swc_common::{SourceMapperDyn, errors::Handler};

struct StringHandlerWriteTarget {
    buffer: Arc<Mutex<Option<Vec<u8>>>>,
}
impl std::io::Write for StringHandlerWriteTarget {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Some(ref mut buffer) = *self.buffer.lock().unwrap() {
            buffer.write(buf)
        } else {
            Err(std::io::Error::new(std::io::ErrorKind::Other, "Buffer has been taken"))
        }
    }
    
    fn flush(&mut self) -> std::io::Result<()> {
        if let Some(ref mut buffer) = *self.buffer.lock().unwrap() {
            buffer.flush()
        } else {
            Err(std::io::Error::new(std::io::ErrorKind::Other, "Buffer has been taken"))
        }
    }
}

pub(crate) struct StringHandlerOutput {
    buffer: Arc<Mutex<Option<Vec<u8>>>>,
}
impl StringHandlerOutput {
    pub fn new(cm: Option<Arc<SourceMapperDyn>>) -> (Handler, Self) {
        let buffer = Arc::new(Mutex::new(Some(Vec::new())));
        let writer = StringHandlerWriteTarget {
            buffer: buffer.clone(),
        };
        (Handler::with_emitter_writer(Box::new(writer), cm), Self {
            buffer,
        })
    }
    pub fn into_string(self) -> String {
        String::from_utf8(self.buffer.lock().unwrap().take().expect("Can only take from string handler output once")).unwrap_or_default()
    }
}
