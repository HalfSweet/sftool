use crate::{ReadFlashParams, ProgressCallback};
use std::future::Future;
use std::pin::Pin;

pub trait ReadFlashTrait {
    fn read_flash<'a>(
        &'a mut self, 
        params: &'a ReadFlashParams,
        progress_callback: Option<ProgressCallback>
    ) -> Pin<Box<dyn Future<Output = Result<(), std::io::Error>> + Send + 'a>>;
}
