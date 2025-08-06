use crate::{EraseFlashParams, EraseRegionParams, ProgressCallback};
use std::future::Future;
use std::pin::Pin;

pub trait EraseFlashTrait {
    fn erase_flash<'a>(
        &'a mut self, 
        params: &'a EraseFlashParams,
        progress_callback: Option<ProgressCallback>
    ) -> Pin<Box<dyn Future<Output = Result<(), std::io::Error>> + Send + 'a>>;
    
    fn erase_region<'a>(
        &'a mut self, 
        params: &'a EraseRegionParams,
        progress_callback: Option<ProgressCallback>
    ) -> Pin<Box<dyn Future<Output = Result<(), std::io::Error>> + Send + 'a>>;
}
