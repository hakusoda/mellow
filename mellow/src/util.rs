pub fn unwrap_string_or_array(value: &serde_json::Value) -> Option<&str> {
	value.as_array().map_or_else(|| value.as_str(), |x| x.get(0).and_then(|x| x.as_str()))
}