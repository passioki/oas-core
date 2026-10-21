//! ONNX text detector (PP-OCRv6 det small). Feature-gated with `onnx`.

use std::path::Path;

use ort::environment::Environment;
use ort::session::Session;
use ort::value::Tensor;

use crate::det::{postprocess_det, preprocess_det};
use crate::RgbCrop;
use oas_types::Rect;

/// Text-line detector: crop in, bounding boxes (source coordinates) out.
pub struct DetEngine {
    _env: &'static Environment,
    session: Session,
    input_name: String,
}

pub(crate) fn global_env() -> Result<&'static Environment, String> {
    static ENV: std::sync::OnceLock<Result<Environment, String>> = std::sync::OnceLock::new();
    ENV.get_or_init(|| ort::init().build().map_err(|e| format!("ort env: {e}")))
        .as_ref()
        .map_err(|e| e.clone())
}

impl DetEngine {
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        let env = global_env()?;
        let session = Session::builder(env)
            .map_err(|e| format!("ort builder: {e}"))?
            .commit_from_file(path)
            .map_err(|e| format!("load {}: {e}", path.display()))?;
        let input_name = session
            .inputs()
            .first()
            .map(|i| i.name().to_string())
            .ok_or_else(|| "det model has no inputs".to_string())?;
        Ok(Self {
            _env: env,
            session,
            input_name,
        })
    }

    /// Detect text lines inside `crop`, returning boxes in crop coordinates.
    pub fn detect(&mut self, crop: &RgbCrop) -> Result<Vec<Rect>, String> {
        let t = preprocess_det(crop.width, crop.height, &crop.data);
        let shape = t.shape();
        let input =
            Tensor::from_array((shape, t.data)).map_err(|e| format!("build det input: {e}"))?;
        let outputs = self
            .session
            .run(ort::inputs![self.input_name.as_str() => input])
            .map_err(|e| format!("det inference: {e}"))?;
        let (out_shape, data) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("det output: {e}"))?;
        let dims: Vec<usize> = out_shape.iter().map(|d| *d as usize).collect();
        let (h, w) = match dims.as_slice() {
            [1, 1, h, w] => (*h, *w),
            [1, h, w] => (*h, *w),
            [h, w] => (*h, *w),
            other => return Err(format!("unexpected det output shape {other:?}")),
        };
        Ok(postprocess_det(
            data,
            w as u32,
            h as u32,
            crop.width,
            crop.height,
        ))
    }
}
