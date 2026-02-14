#![cfg_attr(target_os = "xous", no_std)]
#![cfg_attr(target_os = "xous", no_main)]
extern crate alloc;
mod app; mod sql; mod ui;
use app::SqlApp;
const SERVER_NAME: &str = "sqlite_client";
const APP_NAME: &str = "SQL Explorer";
#[derive(Debug, num_derive::FromPrimitive, num_derive::ToPrimitive)]
pub enum AppOp { Redraw = 0, Rawkeys = 1, FocusChange = 2, Quit = 255 }
fn main() -> ! {
    log_server::init_wait().unwrap();
    log::set_max_level(log::LevelFilter::Info);
    let xns = xous_names::XousNames::new().unwrap();
    let sid = xns.register_name(SERVER_NAME, None).expect("register name");
    let gam = gam::Gam::new(&xns).unwrap();
    let _token = gam.register_ux(gam::UxRegistration {
        app_name: xous::String::<128>::from_str(APP_NAME), ux_type: gam::UxType::Chat,
        predictor: None, listener: sid.to_array(), redraw_id: AppOp::Redraw as u32,
        gotinput_id: None, audioframe_id: None, rawkeys_id: Some(AppOp::Rawkeys as u32),
        focuschange_id: Some(AppOp::FocusChange as u32),
    }).expect("register UX");
    let mut app = SqlApp::new();
    loop {
        let msg = xous::receive_message(sid).unwrap();
        let op: Option<AppOp> = num_traits::FromPrimitive::from_usize(msg.body.id());
        match op {
            Some(AppOp::Redraw) => { app.needs_redraw = true; }
            Some(AppOp::Rawkeys) => {
                let rawkeys = gam::RawkeysEvent::from_message(&msg);
                if let Some(c) = rawkeys.char() {
                    if !app.handle_key(c) { gam.relinquish_focus().unwrap(); xous::wait_thread(core::time::Duration::from_millis(100)); break; }
                }
            }
            Some(AppOp::FocusChange) => { app.needs_redraw = true; }
            Some(AppOp::Quit) => { break; }
            _ => {}
        }
        if app.needs_redraw { ui::draw(&gam, &app); app.needs_redraw = false; }
    }
    xns.unregister_server(sid).unwrap();
    xous::terminate_process(0)
}
