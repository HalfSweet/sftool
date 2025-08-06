use crate::{WriteFlashParams, ProgressCallback};
use std::future::Future;
use std::pin::Pin;

pub trait WriteFlashTrait {
    fn write_flash<'a>(
        &'a mut self, 
        params: &'a WriteFlashParams,
        progress_callback: Option<ProgressCallback>
    ) -> Pin<Box<dyn Future<Output = Result<(), std::io::Error>> + Send + 'a>>;
}
