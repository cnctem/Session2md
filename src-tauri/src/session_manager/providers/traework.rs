use crate::session_manager::SessionMeta;

use super::trae::{self, TraeProduct};

pub fn scan_sessions() -> Vec<SessionMeta> {
    trae::scan_product(TraeProduct::TraeWork)
}
