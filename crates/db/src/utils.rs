use serde::Serialize;

pub trait AsTrimmedStr: Serialize {
    fn as_trimmed_json_string(&self) -> Result<String, serde_json::Error> {
        Ok(serde_json::to_string(self)?.trim_matches('"').to_string())
    }
}
impl<T: ?Sized + Serialize> AsTrimmedStr for T {}
