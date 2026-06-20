use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct KeyboardState {
    state: Option<bool>,
}
