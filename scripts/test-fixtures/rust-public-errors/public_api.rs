pub fn multiline(
    _value: &str,
) -> Result<(), String> {
    Ok(())
}

pub async fn async_multiline(
    _value: &str,
) -> Result<(), String> {
    Ok(())
}

pub type CallbackResult = Result<(), String>;

pub trait Observer {
    fn on_event(
        &self,
    ) -> Result<(), String>;
}

pub fn typed_result() -> Result<String, TypedError> {
    unreachable!()
}

fn private_result() -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod inline_tests {
    pub fn test_helper() -> Result<(), String> {
        Ok(())
    }
}
