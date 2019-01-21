use std::sync::Arc;

use wayland_client::global_filter;
use wayland_client::{EventQueue, Display, GlobalManager, NewProxy, Proxy};

use wayland_client::protocol::wl_display::RequestsTrait as DisplayRequests;
use wayland_client::protocol::wl_registry::RequestsTrait as RegistryRequests;
use wayland_client::protocol::wl_output::RequestsTrait as OutputRequests;
use wayland_client::protocol::{wl_output};

use wayland_protocols::wlr::unstable::export_dmabuf::v1::client::zwlr_export_dmabuf_manager_v1::RequestsTrait as ExportDmabufMngRequests;
use wayland_protocols::wlr::unstable::export_dmabuf::v1::client::zwlr_export_dmabuf_frame_v1::RequestsTrait as ExportDmabufFrameRequests;
use wayland_protocols::wlr::unstable::export_dmabuf::v1::client::{zwlr_export_dmabuf_frame_v1, zwlr_export_dmabuf_manager_v1};

#[derive(Clone)]
struct CaptureContext {
    outputs: Vec<Proxy<wl_output::WlOutput>>,
    dmabuf_mng: Option<Proxy<zwlr_export_dmabuf_manager_v1::ZwlrExportDmabufManagerV1>>,
}

fn main() {
    let (display, mut event_queue) = Display::connect_to_env()
        .expect("Unable to connect to a wayland compositor");
    let globals = GlobalManager::new(&display);
    let registry = display.get_registry(|reg| reg.implement(|_, _| {}, ()))
        .expect("Unable to get registry from display");

    let mut ctx = CaptureContext{
        outputs: Vec::new(),
        dmabuf_mng: None,
    };

    // let _globals = GlobalManager::new_with_cb(
    //     &display,
    //     |global_event, registry| {
    //         use wayland_client::GlobalEvent;
    //         match global_event {
    //             GlobalEvent::New { id, interface, version} => {
    //                 match &*interface {
    //                     "wl_output" => {
    //                         registry.bind::<wl_output::WlOutput, _>(version, id, |output: NewProxy<_>| {
    //                             let output = output.implement(|_, _| {}, ());
    //                             ctx.outputs.push(output.clone());
    //                             output
    //                         })
    //                             .expect(&format!("Unable to bind id={} interface={}", id, interface));
    //                     },
    //                     "zwlr_export_dmabuf_manager_v1" => (),
    //                     _ => (),
    //                 }
    //             }
    //             _ => (),
    //         }
    //     });
        // global_filter!(
        //     [wl_output::WlOutput, 3, |output: NewProxy<_>| {
        //         let output = output.implement(|_, _| {}, ());
        //         ctx.outputs.push(output.clone());
        //         output
        //     }],
        //     [zwlr_export_dmabuf_manager_v1::ZwlrExportDmabufManagerV1, 1, |dmabuf_mng: NewProxy<_>| {
        //         let dmabuf_mng = dmabuf_mng.implement(|_, _| {}, ());
        //         ctx.dmabuf_mng = Some(dmabuf_mng.clone());
        //         dmabuf_mng
        //     }]
        //     ));

    event_queue.sync_roundtrip()
        .expect("event_queue: Failed to sync_roundtrip");

    {
        // dmabuf_mng
        let (id, version) = get_all_wl(&globals, "zwlr_export_dmabuf_manager_v1")
            .into_iter()
            .last()
            .expect("Your compositor doesn't seem to support the wlr_export_dmabuf protocol");
        let dmabuf_mng = registry.bind(version, id, |new_proxy| new_proxy.implement(|_, _| {}, ()))
            .expect("Unable to bind zwlr_export_dmabuf_manager_v1");
        ctx.dmabuf_mng = Some(dmabuf_mng);
    }
    {
        // output
        let (id, version) = get_all_wl(&globals, "wl_output")
            .into_iter()
            .last()
            .expect("No outputs attached?");
        let output = registry.bind(version, id, |new_proxy| new_proxy.implement(|_, _| {}, ()))
            .expect("Unable to bind wl_output");
        ctx.outputs.push(output);
    }

    let dmabuf_mng = ctx.dmabuf_mng.unwrap();
    {
        // capture loop
        dmabuf_mng
            .capture_output(1 , &ctx.outputs.first().unwrap(), |newframe| {
                newframe.implement(
                    |event, frame: Proxy<zwlr_export_dmabuf_frame_v1::ZwlrExportDmabufFrameV1>| {
                        use wayland_protocols::wlr::unstable::export_dmabuf::v1::client::zwlr_export_dmabuf_frame_v1::Event;
                        match event {
                            Event::Frame {
                                width,
                                height,
                                offset_x,
                                offset_y,
                                // buffer_flags,
                                // flags,
                                format,
                                mod_high,
                                mod_low,
                                num_objects,
                                ..
                            } => {
                                // TODO respect flags
                                let format_modifier = (mod_high as u64) << 32 | mod_low as u64;
                                println!("new frame\nsize: {}x{}\noffset: {}x{}\nformat: {} (mod {})\nnum_objects: {}", 
                                         width, height, 
                                         offset_x, offset_y,
                                         format,
                                         format_modifier,
                                         num_objects);
                            },
                            Event::Object { fd, .. } => {
                                use std::fs::File;
                                use std::os::unix::io::FromRawFd;
                                println!("payload");
                                let fd = unsafe { File::from_raw_fd(fd) };
                                drop(fd);
                            }
                            Event::Ready { .. } => {
                                println!("frame ready");
                                frame.destroy();
                            },
                            Event::Cancel { .. } => {
                                println!("frame cancelled");
                                frame.destroy();
                            }
                        }
                    }, 
                    (),
                )
            })
            .expect("Unable to capture output");
        event_queue.sync_roundtrip()
            .expect("event_queue: Failed to sync_roundtrip");
    }
}

fn get_all_wl(globals: &GlobalManager, interface: &str) -> Vec<(u32, u32)> {
    globals
        .list()
        .into_iter()
        // .inspect(|(id, inf, version)| println!("{}: {}, (version {})", id, inf, version))
        .filter(|(_, inf, _)| inf == interface)
        .map(|(id, _, version)| (id, version))
        .collect()
}
