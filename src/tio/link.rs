use std::string::FromUtf8Error;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use thiserror::Error;
use url::Url;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LinkState {
    pub schema: LinkSchema,
    pub domain: LinkDomain,
    pub language: String,
    pub code: String,
    pub input: String,
    pub args: Vec<String>,
    pub debug: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LinkSchema {
    Old,
    #[default]
    New,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LinkDomain {
    /// https://tio.run/
    #[default]
    Tio,
    /// https://tio.run/nexus/
    TioNexus,
    /// http://<language>.tryitonline.net/
    TryItOnline,
}

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("URL parse: {0}")]
    Url(#[from] url::ParseError),
    #[error("unknown domain: {0}")]
    UnknownDomain(String),
    #[error("multiple languages")]
    MultipleLanguages,
    #[error("field value contains `=`")]
    FieldContainsEquals,
    #[error("unknown field: {0}")]
    UnknownField(String),
    #[error("duplicate field: {0}")]
    DuplicateField(String),
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

    /// Decode a Try It Online share link with the old format.
    pub fn decode_old(url: &str) -> Result<Self, DecodeError> {
        let u = Url::parse(url)?;

        let mut language = None;
        let domain = if let Some(domain) = u.domain() {
            if domain == "tio.run" {
                if let Some(l) = u.path().strip_prefix("/nexus/") {
                    language = Some(l.into());
                    LinkDomain::TioNexus
                } else {
                    LinkDomain::Tio
                }
            } else if let Some(l) = domain.strip_suffix(".tryitonline.net") {
                language = Some(l.into());
                LinkDomain::TryItOnline
            } else {
                return Err(DecodeError::UnknownDomain(domain.into()));
            }
        } else {
            return Err(DecodeError::UnknownDomain("".into()));
        };

        let mut fragment = u.fragment().unwrap_or_default();
        if let Some((l, f)) = fragment.split_once('#') {
            if language.is_some() {
                return Err(DecodeError::MultipleLanguages);
            }
            language = Some(l.into());
            fragment = f;
        }

        let mut code = None;
        let mut input = None;
        let mut args = None;
        let mut debug = None;
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
                let b = URL_SAFE_NO_PAD.decode(&*value)?;
                // `escape` with `decodeURIComponent` essentially decodes text
                // as UTF-8.
                let value = String::from_utf8(b)?;
                match key {
                    "code" if code.is_none() => code = Some(value),
                    "input" if input.is_none() => input = Some(value),
                    "args" if args.is_none() => {
                        args = Some(value.split('+').map(str::to_owned).collect())
                    }
                    "debug" if debug.is_none() => {
                        // Any value is treated as "on"
                        debug = Some(true);
                    }
                    "code" | "input" | "args" | "debug" => {
                        return Err(DecodeError::DuplicateField(key.into()))
                    }
                    _ => return Err(DecodeError::UnknownField(key.into())),
                }
            }
        }

        Ok(LinkState {
            schema: LinkSchema::Old,
            domain,
            language: language.unwrap_or_default(),
            code: code.unwrap_or_default(),
            input: input.unwrap_or_default(),
            args: args.unwrap_or_default(),
            debug: debug.unwrap_or_default(),
        })
    }

    /// Encode a Try It Online share link with the old format.
    pub fn encode_old(&self) -> String {
        assert_eq!(self.schema, LinkSchema::Old);
        let mut s = String::new();
        match self.domain {
            LinkDomain::Tio => {
                s.push_str("https://tio.run/#");
                s.push_str(&self.language);
            }
            LinkDomain::TioNexus => {
                s.push_str("https://tio.run/nexus/");
                s.push_str(&self.language);
            }
            LinkDomain::TryItOnline => {
                s.push_str("http://");
                s.push_str(&self.language);
                s.push_str(".tryitonline.net/")
            }
        }
        s.push_str("#code=");
        URL_SAFE_NO_PAD.encode_string(&*self.code, &mut s);
        s.push_str("&input=");
        URL_SAFE_NO_PAD.encode_string(&*self.input, &mut s);
        for (i, arg) in self.args.iter().enumerate() {
            s.push_str(if i == 0 { "&args=" } else { "+" });
            URL_SAFE_NO_PAD.encode_string(&*arg, &mut s);
        }
        if self.debug {
            s.push_str("&debug=on");
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_old() {
        // A link from 2016-09-23
        // https://codegolf.stackexchange.com/questions/44680/showcase-of-languages/93737#93737
        let url = "http://slashes.tryitonline.net/#code=L-KYgy_imIM4L-KYgw&input=";
        let state = LinkState {
            schema: LinkSchema::Old,
            domain: LinkDomain::TryItOnline,
            language: "slashes".into(),
            code: "/☃/☃8/☃".into(),
            input: "".into(),
            args: vec![],
            debug: false,
        };
        assert_eq!(state, LinkState::decode_old(url).unwrap());
        assert_eq!(url, state.encode_old());

        // A link from 2016-11-23, that was updated on 2018-05-26
        // https://codegolf.stackexchange.com/questions/8696/print-this-diamond/100788#100788
        let url = "http://05ab1e.tryitonline.net/#code=OUxKLnDDu3Z5OXlnLcO6w7ss&input=";
        let mut state = LinkState {
            schema: LinkSchema::Old,
            domain: LinkDomain::TryItOnline,
            language: "05ab1e".into(),
            code: "9LJ.pûvy9yg-úû,".into(),
            input: "".into(),
            args: vec![],
            debug: false,
        };
        assert_eq!(state, LinkState::decode_old(url).unwrap());
        assert_eq!(url, state.encode_old());
        let url = "https://tio.run/#05ab1e#code=OUxKLnDDu3Z5OXlnLcO6w7ss&input=";
        state.domain = LinkDomain::Tio;
        assert_eq!(state, LinkState::decode_old(url).unwrap());
        assert_eq!(url, state.encode_old());

        // A link from 2016-12-09, that was updated 2017-04-06
        // https://codegolf.stackexchange.com/questions/99674/three-polyglots-two-period-two-quines-and-one-code-golf-challenge/102533#102533
        let url = "http://retina.tryitonline.net/#code=VT11bmljaHI7cz1VKDM5KSozO189dScnJ1xuI1U9dW5pY2hyO3M9VSgzOSkqMztfPXUlcy5yZXBsYWNlKFUoOSksVSg5NikpO3ByaW50IF8lJShzK18rcykucmVwbGFjZShVKDEwKSxVKDkyKSsnbicpLnJlcGxhY2UoVSg5NiksVSg5KSkucmVwbGFjZShVKDE3OCksVSgxNzkpKS5yZXBsYWNlKFUoMTgzKSxVKDE4NCkpLnJlcGxhY2UoVSgxODIpLFUoMTgzKSkjfMK2I8K3cHJpbnQiV3JvbmcgbGFuZ3VhZ2UhIsK3Iz8uKnR8IsK3wrYjezJ9fF4uwrZcbiMxCSNcblxuI1QJwrItwrkJX28JW17CuV1cbm49Y2hyKDEwKTtwcmludCBuK24uam9pbihbJ3ByaW50Ildyb25nIGxhbmd1YWdlISInLCcjPy4qdHwiJ10pK25cbicnJy5yZXBsYWNlKFUoOSksVSg5NikpO3ByaW50IF8lKHMrXytzKS5yZXBsYWNlKFUoMTApLFUoOTIpKyduJykucmVwbGFjZShVKDk2KSxVKDkpKS5yZXBsYWNlKFUoMTc4KSxVKDE3OSkpLnJlcGxhY2UoVSgxODMpLFUoMTg0KSkucmVwbGFjZShVKDE4MiksVSgxODMpKSN8CiPCtnByaW50Ildyb25nIGxhbmd1YWdlISLCtiM_Lip0fCLCtgojezJ9fF4uCg&input=";
        let mut state = LinkState {
            schema: LinkSchema::Old,
            domain: LinkDomain::TryItOnline,
            language: "retina".into(),
            code: "U=unichr;s=U(39)*3;_=u'''\\n#U=unichr;s=U(39)*3;_=u%s.replace(U(9),U(96));print _%%(s+_+s).replace(U(10),U(92)+'n').replace(U(96),U(9)).replace(U(178),U(179)).replace(U(183),U(184)).replace(U(182),U(183))#|¶#·print\"Wrong language!\"·#?.*t|\"·¶#{2}|^.¶\\n#1	#\\n\\n#T	²-¹	_o	[^¹]\\nn=chr(10);print n+n.join(['print\"Wrong language!\"','#?.*t|\"'])+n\\n'''.replace(U(9),U(96));print _%(s+_+s).replace(U(10),U(92)+'n').replace(U(96),U(9)).replace(U(178),U(179)).replace(U(183),U(184)).replace(U(182),U(183))#|\n#¶print\"Wrong language!\"¶#?.*t|\"¶\n#{2}|^.\n".into(),
            input: "".into(),
            args: vec![],
            debug: false,
        };
        assert_eq!(state, LinkState::decode_old(url).unwrap());
        assert_eq!(url, state.encode_old());
        let url = "https://tio.run/nexus/retina#code=VT11bmljaHI7cz1VKDM5KSozO189dScnJ1xuI1U9dW5pY2hyO3M9VSgzOSkqMztfPXUlcy5yZXBsYWNlKFUoOSksVSg5NikpO3ByaW50IF8lJShzK18rcykucmVwbGFjZShVKDEwKSxVKDkyKSsnbicpLnJlcGxhY2UoVSg5NiksVSg5KSkucmVwbGFjZShVKDE3OCksVSgxNzkpKS5yZXBsYWNlKFUoMTgzKSxVKDE4NCkpLnJlcGxhY2UoVSgxODIpLFUoMTgzKSkjfMK2I8K3cHJpbnQiV3JvbmcgbGFuZ3VhZ2UhIsK3Iz8uKnR8IsK3wrYjezJ9fF4uwrZcbiMxCSNcblxuI1QJwrItwrkJX28JW17CuV1cbm49Y2hyKDEwKTtwcmludCBuK24uam9pbihbJ3ByaW50Ildyb25nIGxhbmd1YWdlISInLCcjPy4qdHwiJ10pK25cbicnJy5yZXBsYWNlKFUoOSksVSg5NikpO3ByaW50IF8lKHMrXytzKS5yZXBsYWNlKFUoMTApLFUoOTIpKyduJykucmVwbGFjZShVKDk2KSxVKDkpKS5yZXBsYWNlKFUoMTc4KSxVKDE3OSkpLnJlcGxhY2UoVSgxODMpLFUoMTg0KSkucmVwbGFjZShVKDE4MiksVSgxODMpKSN8CiPCtnByaW50Ildyb25nIGxhbmd1YWdlISLCtiM_Lip0fCLCtgojezJ9fF4uCg&input=";
        state.domain = LinkDomain::TioNexus;
        assert_eq!(state, LinkState::decode_old(url).unwrap());
        assert_eq!(url, state.encode_old());
    }

    #[test]
    fn roundtrip_all() {
        let links = include_str!("../../tests/tio_links.txt");
        for link in links.lines() {
            let state = LinkState::decode_old(link).unwrap();
            let encoded = state.encode_old();
            if encoded != link {
                // Language-only links like http://cubically.tryitonline.net/
                // encode with empty fragment fields
                if state.domain == LinkDomain::TryItOnline
                    && Url::parse(link).unwrap().fragment().is_some()
                {
                    assert_eq!(encoded, link, "encoding {link}");
                }
            }
        }
    }
}
