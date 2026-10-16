//! Original ABI test fixture, no Adobe sample code or assets. Never a Sapphire replacement.
use after_effects as ae;

#[derive(Default)]
struct Gain;
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
enum Parameter {
    Gain,
}
ae::define_effect!(Gain, (), Parameter);

impl AdobePluginGlobal for Gain {
    fn params_setup(&self, params: &mut Parameters<Parameter>, _: InData, _: OutData) -> Result<(), Error> {
        params.add(
            Parameter::Gain,
            "Gain",
            ae::FloatSliderDef::setup(|value| {
                value.set_default(1.0).set_value(1.0).set_valid_min(0.0).set_valid_max(4.0).set_slider_min(0.0).set_slider_max(4.0);
            }),
        )
    }

    fn handle_command(&mut self, command: Command, in_data: InData, mut out_data: OutData, params: &mut Parameters<Parameter>) -> Result<(), Error> {
        match command {
            Command::GlobalSetup => {
                out_data.set_out_flag2(OutFlags2::SupportsSmartRender, true);
                out_data.set_out_flag2(OutFlags2::FloatColorAware, true);
            }
            Command::SmartPreRender { mut extra } => {
                let request = extra.output_request();
                let result = extra.callbacks().checkout_layer(0, 0, &request, in_data.current_time(), in_data.time_step(), in_data.time_scale())?;
                extra.set_result_rect(result.result_rect.into());
                extra.set_max_result_rect(result.max_result_rect.into());
            }
            Command::SmartRender { extra } => {
                let cb = extra.callbacks();
                let input = cb.checkout_layer_pixels(0)?.ok_or(Error::BadCallbackParameter)?;
                let mut output = cb.checkout_output()?.ok_or(Error::BadCallbackParameter)?;
                let gain = params.get(Parameter::Gain)?.as_float_slider()?.value() as f32;
                if gain == 4.0 {
                    std::process::exit(71);
                } // Deliberate child exit for recovery regression.
                let gain = gain + in_data.current_timestamp() as f32 * 0.1;
                input.iterate_with(&mut output, 0, input.height() as i32, None, |_, _, input, output| match (input, output) {
                    (GenericPixel::PixelF32(input), GenericPixelMut::PixelF32(output)) => {
                        output.alpha = input.alpha;
                        output.red = input.red * gain;
                        output.green = input.green * gain;
                        output.blue = input.blue * gain;
                        Ok(())
                    }
                    _ => Err(Error::BadCallbackParameter),
                })?;
                cb.checkin_layer_pixels(0)?;
            }
            _ => {}
        }
        Ok(())
    }
}
