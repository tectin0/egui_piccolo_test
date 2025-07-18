use anyhow::anyhow;
use gc_arena::{Collect, Gc, Rootable, lock::Lock};
use piccolo::{Callback, CallbackReturn, Closure, Executor, Lua, RuntimeError, UserData, Value};

use crate::frame_history::FrameHistory;

pub struct TemplateApp {
    code: String,
    lua: Lua,
    frame_history: FrameHistory,
}

impl TemplateApp {
    pub fn new(_: &eframe::CreationContext<'_>) -> Self {
        let lua = Lua::full();
        let code = String::default();
        let frame_history = FrameHistory::default();

        Self {
            code,
            lua,
            frame_history,
        }
    }
}

#[derive(Collect)]
#[collect(no_drop)]
struct Lui<'gc>(Gc<'gc, Lock<u64>>);

impl eframe::App for TemplateApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.frame_history
            .on_new_frame(ctx.input(|i| i.time), frame.info().cpu_usage);

        egui::CentralPanel::default().show(ctx, |ui| {
            self.frame_history.ui(ui);
            ui.label(format!("FPS: {:.2}", self.frame_history.fps()));
        });

        egui::Window::new("Lua").show(ctx, |ui| {
            ui.text_edit_multiline(&mut self.code);

            let lua = &mut self.lua;

            let ex = lua.try_enter(|ctx| {
                let lui = UserData::new::<Rootable![Lui<'_>]>(
                    &ctx,
                    Lui(Gc::new(&ctx, Lock::new(std::ptr::from_ref(ui) as u64))),
                );

                ctx.set_global("lui", lui)
                    .expect("Failed to set global 'lui'");

                let label = Callback::from_fn(&ctx, |_, _, mut stack| {
                    #[expect(clippy::single_match_else)]
                    match stack[0] {
                        Value::UserData(ud) => {
                            let ud = ud
                                .downcast::<Rootable![Lui<'_>]>()
                                .expect("Failed to downcast");

                            // SAFETY: We know that `ud.0` is a `Lock<u64>` that points to an `egui::Ui`.
                            let ui = unsafe { &mut *(ud.0.get() as *mut u64).cast::<egui::Ui>() };

                            let label = match stack.get(1) {
                                Value::String(s) => s.to_string(),
                                _ => {
                                    return Err(piccolo::Error::Runtime(RuntimeError(
                                        anyhow!("Expected a string for the label",).into(),
                                    )));
                                }
                            };

                            ui.label(label);
                        }
                        _ => panic!(),
                    };

                    stack.clear();

                    Ok(CallbackReturn::Return)
                });

                ctx.set_global("label", label)
                    .expect("Failed to set global 'label'");

                let button = Callback::from_fn(&ctx, |_, _, mut stack| {
                    #[expect(clippy::single_match_else)]
                    match stack[0] {
                        Value::UserData(ud) => {
                            let ud = ud
                                .downcast::<Rootable![Lui<'_>]>()
                                .expect("Failed to downcast");

                            // SAFETY: We know that `ud.0` is a `Lock<u64>` that points to an `egui::Ui`.
                            let ui = unsafe { &mut *(ud.0.get() as *mut u64).cast::<egui::Ui>() };

                            let label = match stack.get(1) {
                                Value::String(s) => s.to_string(),
                                _ => {
                                    return Err(piccolo::Error::Runtime(RuntimeError(
                                        anyhow!("Expected a string for the button label",).into(),
                                    )));
                                }
                            };

                            let _response = ui.button(label);
                        }
                        _ => panic!(),
                    };

                    stack.clear();

                    let ok: Result<CallbackReturn<'_>, piccolo::Error<'_>> =
                        Ok(CallbackReturn::Return);

                    ok
                });

                ctx.set_global("button", button)
                    .expect("Failed to set global 'button'");

                let env = ctx.globals();

                let closure = Closure::load_with_env(ctx, None, self.code.as_bytes(), env)?;

                let ex = Executor::start(ctx, closure.into(), "this is my message");

                Ok(ctx.stash(ex))
            });

            let ex = match ex {
                Ok(ex) => ex,
                Err(err) => {
                    ui.label(format!("Error loading Lua code: {err}"));
                    return;
                }
            };

            match lua.execute::<()>(&ex) {
                Ok(result) => result,
                Err(err) => {
                    ui.label(format!("Error executing Lua code: {err}"));
                    return;
                }
            };

            ui.label("Lua code executed successfully!");
        });
    }
}
