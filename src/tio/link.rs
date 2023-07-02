use std::string::FromUtf8Error;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use thiserror::Error;
use url::Url;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LinkState {
    pub schema: LinkSchema,
    pub code: String,
    pub input: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LinkSchema {
    Old,
    #[default]
    New,
}

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("URL parse: {0}")]
    Url(#[from] url::ParseError),
    #[error("no URL fragment")]
    NoFragment,
    #[error("field value contains `=`")]
    FieldContainsEquals,
    #[error("unknown field: {0}")]
    UnknownField(String),
    #[error("duplicate field: {0}")]
    DuplicateField(String),
    #[error("missing field: {0}")]
    MissingField(String),
    #[error("base64 decode: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("UTF-8 decode: {0}")]
    Utf8(#[from] FromUtf8Error),
}

#[derive(Debug, Error)]
pub enum EncodeError {}

impl LinkState {
    pub fn new() -> Self {
        LinkState::default()
    }

    /// Decode a Try It Online share link with the format from the first commit,
    /// [fa68bea](https://github.com/TryItOnline/tryitonline/blob/fa68bea9b99541ebca9c48a92874fb461dbe70d6/html/frontend.js)
    /// on 2015-11-20.
    pub fn decode_old(url: &str) -> Result<Self, DecodeError> {
        let u = Url::parse(url)?;
        let fragment = u.fragment().ok_or(DecodeError::NoFragment)?;
        let mut code = None;
        let mut input = None;
        for field in fragment.split('&') {
            if let Some((key, value)) = field.split_once('=') {
                if value.contains('=') {
                    // TIO ignores anything after another `=`, but it is never
                    // encoded like this, so error.
                    return Err(DecodeError::FieldContainsEquals);
                }
                // TIO's base64 decoding allows `+` and `/` from the standard
                // alphabet intermixed with `-` and `_` from the URL-safe
                // alphabet, but the encoder uses URL-safe.
                let value = URL_SAFE_NO_PAD.decode(&*value)?;
                // decodeURIComponent decodes `%XX`-escaped UTF-8.
                let value = String::from_utf8(value)?;
                match key {
                    "code" if code.is_none() => code = Some(value),
                    "input" if input.is_none() => input = Some(value),
                    "code" | "input" => return Err(DecodeError::DuplicateField(key.into())),
                    _ => return Err(DecodeError::UnknownField(key.into())),
                }
            }
        }
        let code = code.ok_or_else(|| DecodeError::MissingField("code".into()))?;
        let input = input.ok_or_else(|| DecodeError::MissingField("input".into()))?;
        Ok(LinkState {
            schema: LinkSchema::Old,
            code,
            input,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_format() {
        // https://codegolf.stackexchange.com/questions/44680/showcase-of-languages/93737#93737
        let url = "http://slashes.tryitonline.net/#code=L-KYgy_imIM4L-KYgw&input=";
        let state = LinkState {
            schema: LinkSchema::Old,
            code: "/☃/☃8/☃".into(),
            input: "".into(),
        };
        assert_eq!(state, LinkState::decode_old(url).unwrap());

        // https://codegolf.stackexchange.com/questions/8696/print-this-diamond/100788#100788
        let url = "https://tio.run/#05ab1e#code=OUxKLnDDu3Z5OXlnLcO6w7ss&input=";
        println!("{:?}", LinkState::decode_old(url));

        // https://codegolf.stackexchange.com/questions/99674/three-polyglots-two-period-two-quines-and-one-code-golf-challenge/102533#102533
        let url = "https://tio.run/nexus/retina#code=VT11bmljaHI7cz1VKDM5KSozO189dScnJ1xuI1U9dW5pY2hyO3M9VSgzOSkqMztfPXUlcy5yZXBsYWNlKFUoOSksVSg5NikpO3ByaW50IF8lJShzK18rcykucmVwbGFjZShVKDEwKSxVKDkyKSsnbicpLnJlcGxhY2UoVSg5NiksVSg5KSkucmVwbGFjZShVKDE3OCksVSgxNzkpKS5yZXBsYWNlKFUoMTgzKSxVKDE4NCkpLnJlcGxhY2UoVSgxODIpLFUoMTgzKSkjfMK2I8K3cHJpbnQiV3JvbmcgbGFuZ3VhZ2UhIsK3Iz8uKnR8IsK3wrYjezJ9fF4uwrZcbiMxCSNcblxuI1QJwrItwrkJX28JW17CuV1cbm49Y2hyKDEwKTtwcmludCBuK24uam9pbihbJ3ByaW50Ildyb25nIGxhbmd1YWdlISInLCcjPy4qdHwiJ10pK25cbicnJy5yZXBsYWNlKFUoOSksVSg5NikpO3ByaW50IF8lKHMrXytzKS5yZXBsYWNlKFUoMTApLFUoOTIpKyduJykucmVwbGFjZShVKDk2KSxVKDkpKS5yZXBsYWNlKFUoMTc4KSxVKDE3OSkpLnJlcGxhY2UoVSgxODMpLFUoMTg0KSkucmVwbGFjZShVKDE4MiksVSgxODMpKSN8CiPCtnByaW50Ildyb25nIGxhbmd1YWdlISLCtiM_Lip0fCLCtgojezJ9fF4uCg&input=";
        let state = LinkState {
            schema: LinkSchema::Old,
            code: "U=unichr;s=U(39)*3;_=u'''\\n#U=unichr;s=U(39)*3;_=u%s.replace(U(9),U(96));print _%%(s+_+s).replace(U(10),U(92)+'n').replace(U(96),U(9)).replace(U(178),U(179)).replace(U(183),U(184)).replace(U(182),U(183))#|¶#·print\"Wrong language!\"·#?.*t|\"·¶#{2}|^.¶\\n#1	#\\n\\n#T	²-¹	_o	[^¹]\\nn=chr(10);print n+n.join(['print\"Wrong language!\"','#?.*t|\"'])+n\\n'''.replace(U(9),U(96));print _%(s+_+s).replace(U(10),U(92)+'n').replace(U(96),U(9)).replace(U(178),U(179)).replace(U(183),U(184)).replace(U(182),U(183))#|\n#¶print\"Wrong language!\"¶#?.*t|\"¶\n#{2}|^.\n".into(),
            input: "".into(),
        };
        assert_eq!(state, LinkState::decode_old(url).unwrap());
    }
}
