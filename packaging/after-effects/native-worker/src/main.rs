//! Standalone native ABI boundary. The editor does not link aexlo or plugin libraries.
use aexlo::{Depth32, Host, Layer, ParamValue, Pixel};
use serde::Deserialize;
use serde_json::json;
use std::{fs, io::Read, path::PathBuf};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    protocol: u32,
    operation: String,
    plugin: PathBuf,
    #[serde(default)]
    width: u32,
    #[serde(default)]
    height: u32,
    #[serde(default)]
    time: f64,
    #[serde(default)]
    params: Vec<(usize, f64)>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("AEX worker: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let request_path = std::env::args_os().nth(1).ok_or("request path missing")?;
    let response_path = PathBuf::from(std::env::args_os().nth(2).ok_or("response path missing")?);
    let directory = response_path.parent().ok_or("response directory missing")?;
    let mut bytes = Vec::new();
    fs::File::open(request_path)?.take(1024 * 1024 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > 1024 * 1024 {
        return Err("request exceeds 1 MiB".into());
    }
    let request: Request = serde_json::from_slice(&bytes)?;
    if request.protocol != 1 || !request.plugin.is_absolute() || request.params.len() > 256 || !matches!(request.operation.as_str(), "probe" | "render") {
        return Err("invalid worker protocol, operation or plugin path".into());
    }
    let mut instance = Host::get().try_load(&request.plugin)?;
    if !instance.supports_smart_render() {
        return Err("32-float runtime requires declared Smart Render support".into());
    }
    if instance.param_count() > 257 {
        return Err("plugin exceeds 256 parameter limit".into());
    }
    let mut parameters = Vec::new();
    for index in 1..instance.param_count() {
        let value = match instance.get_param(index) {
            Some(ParamValue::Float(value)) => value,
            _ => return Err(format!("parameter {index} has an unsupported native type; only float sliders are enabled in this first runtime").into()),
        };
        let (min, max) = instance.param_slider_range(index).unwrap_or((-1_000_000.0, 1_000_000.0));
        if !value.is_finite() || !min.is_finite() || !max.is_finite() || min > value || value > max {
            return Err("invalid native parameter range".into());
        }
        parameters
            .push(json!({"index":index,"name":instance.param_name(index).unwrap_or_else(|| format!("Parameter {index}")),"default":value,"min":min,"max":max}));
    }
    let mut rendered = false;
    if request.operation == "render" {
        let count = u64::from(request.width).checked_mul(u64::from(request.height)).ok_or("frame size overflow")?;
        if count == 0 || count > 4_194_304 || request.width > 8192 || request.height > 8192 || !request.time.is_finite() || request.time.abs() > 2_000_000.0 {
            return Err("unsupported frame size or time".into());
        }
        let size = usize::try_from(count.checked_mul(16).ok_or("frame byte size overflow")?)?;
        let mut input = Vec::new();
        fs::File::open(directory.join("input.rgba32f"))?.take(size as u64 + 1).read_to_end(&mut input)?;
        if input.len() != size {
            return Err("frame payload length mismatch".into());
        }
        let mut pixels = Vec::with_capacity(usize::try_from(count)?);
        for pixel in input.chunks_exact(16) {
            let mut rgba = [0.0; 4];
            for (value, channel) in rgba.iter_mut().zip(pixel.chunks_exact(4)) {
                *value = f32::from_le_bytes(channel.try_into()?);
                if !value.is_finite() {
                    return Err("non-finite input pixel".into());
                }
            }
            pixels.push(Pixel::<Depth32>::from_unit(rgba));
        }
        instance.set_render_size(request.width, request.height);
        instance.set_input_layer(Layer::<Depth32>::new(request.width, request.height, pixels)?);
        instance.set_time((request.time * 1000.0).round() as i32, 1, 1000);
        for (index, value) in request.params {
            if !value.is_finite()
                || !parameters.iter().any(|p| {
                    p["index"].as_u64() == Some(index as u64)
                        && p["min"].as_f64().is_some_and(|min| value >= min)
                        && p["max"].as_f64().is_some_and(|max| value <= max)
                })
            {
                return Err("invalid render parameter".into());
            }
            instance.set_param(index, ParamValue::Float(value))?;
        }
        instance.render_pre()?;
        instance.render_smart()?;
        let output = instance.read_output::<Depth32>()?;
        if output.width() != request.width || output.height() != request.height {
            return Err("plugin changed unsupported output geometry".into());
        }
        let mut data = Vec::with_capacity(size);
        for pixel in output.pixels() {
            for value in pixel.to_unit() {
                if !value.is_finite() {
                    return Err("non-finite output pixel".into());
                }
                data.extend_from_slice(&value.to_le_bytes());
            }
        }
        fs::write(directory.join("output.rgba32f"), data)?;
        rendered = true;
    }
    fs::write(
        response_path,
        serde_json::to_vec(
            &json!({"protocol":1,"loaded":true,"initialized":true,"parameters":parameters,"rendered":rendered,"depth":32,"fidelityVerified":false}),
        )?,
    )?;
    Ok(())
}
