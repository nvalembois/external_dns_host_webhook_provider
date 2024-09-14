use salvo::prelude::*;
use tracing::debug;

#[handler]
pub async fn get_healthz(res: &mut Response) {
    debug!("get_health");
    res.render(Text::Plain("Ok!"));
    res.status_code(StatusCode::OK);
}

