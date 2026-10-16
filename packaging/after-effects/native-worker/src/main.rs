//! Standalone native ABI boundary. The editor does not link aexlo or plugin libraries.
use aexlo::{Depth32, Host, Layer, ParamValue, Pixel, PixelDepthKind};
use serde::Deserialize;
use serde_json::json;
use std::{fs, io::Read, path::PathBuf};

struct Logger;
impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Warn
    }
    fn log(&self, record: &log::Record<'_>) {
        if self.enabled(record.metadata()) {
            eprintln!("{}: {}", record.level(), record.args().to_string().chars().take(2048).collect::<String>());
        }
    }
    fn flush(&self) {}
}
static LOGGER: Logger = Logger;

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
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(log::LevelFilter::Warn);
    if let Err(error) = run() {
        // Preserve the last completed stage even when a later command fails.
        if let Some(path) = std::env::args_os().nth(2)
            && let Ok(data) = read_checkpoint(&PathBuf::from(&path))
            && let Ok(mut response) = serde_json::from_slice::<serde_json::Value>(&data)
        {
            response["error"] = json!(error.to_string().chars().take(4096).collect::<String>());
            if let Ok(data) = serde_json::to_vec(&response) {
                let _ = fs::write(path, data);
            }
        }
        eprintln!("AEX worker: {error}");
        std::process::exit(1);
    }
}

fn read_checkpoint(path: &std::path::Path) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut data = Vec::new();
    fs::File::open(path)?.take(1024 * 1024 + 1).read_to_end(&mut data)?;
    if data.len() > 1024 * 1024 {
        return Err("checkpoint exceeds 1 MiB".into());
    }
    Ok(data)
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
    if request.protocol != 1
        || !request.plugin.is_absolute()
        || request.params.len() > 256
        || !matches!(request.operation.as_str(), "probe" | "render" | "diagnose" | "identify")
    {
        return Err("invalid worker protocol, operation or plugin path".into());
    }
    let mut response = json!({"protocol":1,"loaded":false,"initialized":false,"parameters":[],"rendered":false,"depth":32,"fidelityVerified":false,"stage":"load/entrypoint","identity":null});
    let checkpoint = |response: &serde_json::Value| -> Result<(), Box<dyn std::error::Error>> {
        fs::write(&response_path, serde_json::to_vec(response)?)?;
        Ok(())
    };
    checkpoint(&response)?;
    let mut instance = Host::get().edition_open(&request.plugin)?;
    response["loaded"] = json!(true);
    if let Some((name, match_name, category, kind)) = instance.edition_registration() {
        response["identity"] = json!({"source":"PluginDataEntryFunction2","name":name,"matchName":match_name,"category":category,"kind":if kind == 1699105620 {"eFKT"} else {"other"},"versionRaw":null});
    }
    response["entryPoint"] = json!(instance.edition_entry_point());
    if request.operation == "identify" {
        response["stage"] = json!("identified");
        checkpoint(&response)?;
        return Ok(());
    }
    if response["identity"].as_object().is_some_and(|_| response["identity"]["kind"] != "eFKT") {
        return Err("registration identifies a non-effect extension; no effect initialization was attempted".into());
    }
    response["stage"] = json!("globalSetup");
    checkpoint(&response)?;
    instance.edition_global_setup()?;
    if !response["identity"].is_null() {
        response["identity"]["versionRaw"] = json!(instance.edition_version());
    }
    response["globalSetup"] = json!(true);
    response["stage"] = json!("paramsSetup");
    checkpoint(&response)?;
    instance.edition_params_setup()?;
    response["paramsSetup"] = json!(true);
    if instance.param_count() > 257 {
        return Err("plugin exceeds 256 parameter limit".into());
    }
    let mut parameters = Vec::new();
    let mut native_parameters = Vec::new();
    let mut all_float = true;
    for index in 1..instance.param_count() {
        let current = match instance.get_param(index) {
            Some(ParamValue::Float(v)) => json!(v),
            Some(ParamValue::Fixed(v)) => json!(v),
            Some(ParamValue::Slider(v) | ParamValue::Popup(v)) => json!(v),
            Some(ParamValue::Checkbox(v)) => json!(v),
            Some(ParamValue::Angle(v)) => json!(v),
            Some(ParamValue::Point { x, y }) => json!([x, y]),
            Some(ParamValue::Point3D { x, y, z }) => json!([x, y, z]),
            Some(ParamValue::Color { red, green, blue, alpha }) => json!([red, green, blue, alpha]),
            Some(ParamValue::Path(v)) => json!(v),
            None => serde_json::Value::Null,
        };
        native_parameters.push(json!({"index":index,"diskId":instance.edition_param_id(index),"name":instance.param_name(index),"kind":format!("{:?}",instance.param_kind(index)),"currentValue":current,"sliderRange":instance.param_slider_range(index),"choices":instance.param_choices(index)}));
        let value = match instance.get_param(index) {
            Some(ParamValue::Float(value)) => value,
            _ => {
                all_float = false;
                continue;
            }
        };
        let (min, max) = instance.param_slider_range(index).unwrap_or((-1_000_000.0, 1_000_000.0));
        if !value.is_finite() || !min.is_finite() || !max.is_finite() || min > value || value > max {
            return Err("invalid native parameter range".into());
        }
        parameters
            .push(json!({"index":index,"name":instance.param_name(index).unwrap_or_else(|| format!("Parameter {index}")),"default":value,"min":min,"max":max}));
    }
    response["parameters"] = json!(parameters);
    response["nativeParameters"] = json!(native_parameters);
    response["editorParameterTypesSupported"] = json!(all_float);
    response["stage"] = json!("sequenceSetup");
    checkpoint(&response)?;
    instance.edition_sequence_setup()?;
    response["initialized"] = json!(true);
    response["smartRender"] = json!(instance.supports_smart_render());
    response["floatColorAware"] = json!(instance.supports_depth(PixelDepthKind::F32));
    response["stage"] = json!("capabilityCheck");
    checkpoint(&response)?;
    if !instance.supports_smart_render() || !instance.supports_depth(PixelDepthKind::F32) {
        return Err("32-float runtime requires declared Smart Render and Float Color Aware support".into());
    }
    if !all_float && request.operation != "diagnose" {
        return Err(
            "native parameters were enumerated, but the editor currently supports only float sliders; use the diagnostic for a defaults-only render attempt"
                .into(),
        );
    }
    let mut rendered = false;
    if matches!(request.operation.as_str(), "render" | "diagnose") {
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
        response["stage"] = json!("smartPreRender");
        checkpoint(&response)?;
        instance.render_pre()?;
        response["smartPreRender"] = json!(true);
        response["stage"] = json!("smartRender");
        checkpoint(&response)?;
        instance.render_smart()?;
        response["stage"] = json!("readback");
        checkpoint(&response)?;
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
    response["rendered"] = json!(rendered);
    response["stage"] = json!("complete");
    checkpoint(&response)?;
    Ok(())
}
