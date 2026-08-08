use rusty_engine::{
    entity_state, render_host_contracts, render_model, render_presentation,
    renderer_webview_host,
};

fn exact_render_frame(frame: render_model::RenderFrameDiff) -> render_model::RenderFrameDiff {
    frame
}

fn exact_presentation_frame(
    frame: render_presentation::PresentationFrameDiff,
) -> render_presentation::PresentationFrameDiff {
    frame
}

fn main() {
    let _entity_state_type = std::any::TypeId::of::<entity_state::EntityState>();
    let _host_options = renderer_webview_host::RendererWebviewOptions::default();
    let _camera = render_host_contracts::RendererCameraPose {
        position: [0.0, 1.62, 8.0],
        pitch_degrees: 0.0,
        yaw_degrees: 0.0,
    };
    let _ = exact_render_frame(render_model::RenderFrameDiff::new());
    let _ = exact_presentation_frame(render_presentation::PresentationFrameDiff::new());
    println!("RUSTY_ENGINE_SINGLE_DEPENDENCY_OK");
}
