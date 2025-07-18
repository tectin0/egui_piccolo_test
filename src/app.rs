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
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let lua = Lua::full();
        let code = "".to_string();
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
        });

        egui::Window::new("Lua").show(ctx, |ui| {
            ui.text_edit_multiline(&mut self.code);

            let lua = &mut self.lua;

            let ex = lua.try_enter(|ctx| {
                let lui = UserData::new::<Rootable![Lui<'_>]>(
                    &ctx,
                    Lui(Gc::new(&ctx, Lock::new(ui as *const _ as u64))),
                );

                ctx.set_global("lui", lui).unwrap();

                let label = Callback::from_fn(&ctx, |ctx, _, mut stack| {
                    match stack[0] {
                        Value::UserData(ud) => {
                            let ud = ud.downcast::<Rootable![Lui<'_>]>().unwrap();

                            let ui = unsafe { &mut *(ud.0.get() as *mut u64 as *mut egui::Ui) };

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

                ctx.set_global("label", label);

                let button = Callback::from_fn(&ctx, |ctx, _, mut stack| {
                    match stack[0] {
                        Value::UserData(ud) => {
                            let ud = ud.downcast::<Rootable![Lui<'_>]>().unwrap();

                            let ui = unsafe { &mut *(ud.0.get() as *mut u64 as *mut egui::Ui) };

                            let label = match stack.get(1) {
                                Value::String(s) => s.to_string(),
                                _ => {
                                    return Err(piccolo::Error::Runtime(RuntimeError(
                                        anyhow!("Expected a string for the button label",).into(),
                                    )));
                                }
                            };

                            ui.button(label);
                        }
                        _ => panic!(),
                    };

                    stack.clear();

                    let ok: Result<CallbackReturn<'_>, piccolo::Error<'_>> =
                        Ok(CallbackReturn::Return);

                    ok
                });

                ctx.set_global("button", button);

                let env = ctx.globals();

                let closure = Closure::load_with_env(ctx, None, self.code.as_bytes(), env)?;

                let ex = Executor::start(ctx, closure.into(), "this is my message");

                Ok(ctx.stash(ex))
            });

            let ex = match ex {
                Ok(ex) => ex,
                Err(err) => {
                    ui.label(format!("Error loading Lua code: {}", err));
                    return;
                }
            };

            let result = match lua.execute::<()>(&ex) {
                Ok(result) => result,
                Err(err) => {
                    ui.label(format!("Error executing Lua code: {}", err));
                    return;
                }
            };

            ui.label(format!("Lua code executed successfully: {:?}", result));
        });
    }
}
