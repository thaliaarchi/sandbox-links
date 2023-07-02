use std::string::FromUtf8Error;

use base64::{
    engine::general_purpose::{STANDARD_NO_PAD, URL_SAFE_NO_PAD},
    Engine,
};
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
    V1,
    #[default]
    V2,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LinkDomain {
    /// TIO v2 (https://tio.run/)
    #[default]
    Tio,
    /// TIO Nexus (https://tio.run/nexus/)
    TioNexus,
    /// TIO v1 (http://<language>.tryitonline.net/)
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

    /// Decode a Try It Online share link with the v1 format.
    pub fn decode_v1(url: &str) -> Result<Self, DecodeError> {
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
            } else if domain == "tryitonline.net" {
                LinkDomain::TryItOnline
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
                match key {
                    "code" if code.is_none() => code = Some(decode_field(value)?),
                    "input" if input.is_none() => input = Some(decode_field(value)?),
                    "args" if args.is_none() => {
                        let a = value
                            .split('+')
                            .map(decode_field)
                            .collect::<Result<_, DecodeError>>()?;
                        args = Some(a);
                    }
                    "debug" if debug.is_none() => debug = Some(true),
                    "code" | "input" | "args" | "debug" => {
                        return Err(DecodeError::DuplicateField(key.into()));
                    }
                    _ => return Err(DecodeError::UnknownField(key.into())),
                }
            }
        }

        Ok(LinkState {
            schema: LinkSchema::V1,
            domain,
            language: language.unwrap_or_default(),
            code: code.unwrap_or_default(),
            input: input.unwrap_or_default(),
            args: args.unwrap_or_default(),
            debug: debug.unwrap_or_default(),
        })
    }

    /// Encode a Try It Online share link with the v1 format.
    pub fn encode_v1(&self) -> String {
        assert_eq!(self.schema, LinkSchema::V1);
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
            LinkDomain::TryItOnline if self.language == "" => s.push_str("http://tryitonline.net/"),
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

fn decode_field(s: &str) -> Result<String, DecodeError> {
    // TIO's base64 decoding allows `+` and `/` from the standard alphabet
    // intermixed with `-` and `_` from the URL-safe alphabet, but the encoder
    // uses URL-safe.
    let b = match URL_SAFE_NO_PAD.decode(&*s) {
        Ok(b) => b,
        // Some links inexplicably use `+`; however, I cannot find when this was
        // ever the case in the code.
        Err(err) => STANDARD_NO_PAD.decode(&*s).map_err(|_| err)?,
    };
    // `escape` with `decodeURIComponent` essentially decodes text as UTF-8.
    Ok(String::from_utf8(b)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_v1() {
        // A link from 2016-09-23
        // https://codegolf.stackexchange.com/questions/44680/showcase-of-languages/93737#93737
        let url = "http://slashes.tryitonline.net/#code=L-KYgy_imIM4L-KYgw&input=";
        let state = LinkState {
            schema: LinkSchema::V1,
            domain: LinkDomain::TryItOnline,
            language: "slashes".into(),
            code: "/☃/☃8/☃".into(),
            input: "".into(),
            args: vec![],
            debug: false,
        };
        assert_eq!(state, LinkState::decode_v1(url).unwrap());
        assert_eq!(url, state.encode_v1());

        // A link with args from 2016-12-20
        // https://codegolf.stackexchange.com/questions/103822/output-the-sign/103927#103927
        let url = "http://v.tryitonline.net/#code=w6kKw4DDqTEyfERrSsOyaMOpLcOyw7NeJC8SYQ&input=&args=LS0+LTY";
        let state = LinkState {
            schema: LinkSchema::V1,
            domain: LinkDomain::TryItOnline,
            language: "v".into(),
            code: "é\nÀé12|DkJòhé-òó^$/\u{0012}a".into(),
            input: "".into(),
            args: vec!["--".into(), "-6".into()],
            debug: false,
        };
        assert_eq!(state, LinkState::decode_v1(url).unwrap());
        assert_eq!(url, state.encode_v1());

        // A link with debug from 2016-09-10
        // https://codegolf.stackexchange.com/questions/92776/esolang-comment-template-generator/92881#92881
        let url = "http://golfscript.tryitonline.net/#code=eyg7KTt9OnI7IiwgIiUoclwociBuKzpjOzsuLFwnfCclLixAXC0pKTpsOzA6bTt7LiwwPn17IiAibSpcKC4sbSs6bSBsXC0iICJcKitjQH13aGlsZQ&input=IkhlbGxvLHxXb3JsZCF8VGhpc3xpc3xHb2xmU2NyaXB0IiwgIiMi&debug=on";
        let state = LinkState {
            schema: LinkSchema::V1,
            domain: LinkDomain::TryItOnline,
            language: "golfscript".into(),
            code: "{(;);}:r;\", \"%(r\\(r n+:c;;.,\\'|'%.,@\\-)):l;0:m;{.,0>}{\" \"m*\\(.,m+:m l\\-\" \"\\*+c@}while".into(),
            input: "\"Hello,|World!|This|is|GolfScript\", \"#\"".into(),
            args: vec![],
            debug: true,
        };
        assert_eq!(state, LinkState::decode_v1(url).unwrap());
        assert_eq!(url, state.encode_v1());

        // A link with debug from 2016-12-03
        // https://codegolf.stackexchange.com/questions/100470/what-will-you-bring-for-thanksgiving/101196#101196
        let url = "http://05ab1e.tryitonline.net/#code=Iz4hPlwnblwnclwnb1wnY1wsLCwsLEAncydhJ2wnYSdkSizDqSdnJ24rJ2krJ3MrJ3MrJ2UrJ3IrJ2QrLsOzWy1dK1stLS0tLT4rKys8XT4uKysrKysrKysrKysuKysrWy0-KysrPF0-KysuK1stLS0-KzxdPi4rKytIJ3R1cmtleSd-O29vb29vb29vJ3N3ZWV0dGVhLjAtNSsrKysrKysrZmZmZmZmZmZmJydgJwojICAgPjk5KmQxK2MzLWM4OSo0K2RjYzk5KjIrYyF8JGw5RDMtTzkzKytkT084K08xK08xLU81K09PMStPOTUrK08uCiMgPidwJygpJ3UnKCknbScoKSdwJygpJ2snKCknaScoKSduJygpJ3AnKCknaScoKSdlJygpXApwcmludCdiaXNjdWl0cyc7J3BlY2FucGllJyMic3R1ZmZpbmciUidjISdvISdyISduISdiISdyISdlISdhISdkISobZGRkZGRkU2FwcGxlcGll&input=&debug=on";
        let state = LinkState {
            schema: LinkSchema::V1,
            domain: LinkDomain::TryItOnline,
            language: "05ab1e".into(),
            code: "#>!>\\'n\\'r\\'o\\'c\\,,,,,@'s'a'l'a'dJ,é'g'n+'i+'s+'s+'e+'r+'d+.ó[-]+[----->+++<]>.+++++++++++.+++[->+++<]>++.+[--->+<]>.+++H'turkey'~;oooooooo'sweettea.0-5++++++++fffffffff''`'\n#   >99*d1+c3-c89*4+dcc99*2+c!|$l9D3-O93++dOO8+O1+O1-O5+OO1+O95++O.\n# >'p'()'u'()'m'()'p'()'k'()'i'()'n'()'p'()'i'()'e'()\\\nprint'biscuits';'pecanpie'#\"stuffing\"R'c!'o!'r!'n!'b!'r!'e!'a!'d!*\u{001b}ddddddSapplepie".into(),
            input: "".into(),
            args: vec![],
            debug: true,
        };
        assert_eq!(state, LinkState::decode_v1(url).unwrap());
        assert_eq!(url, state.encode_v1());
    }

    #[test]
    fn changed_domain() {
        // A link from 2016-11-23, that was updated on 2018-05-26
        // https://codegolf.stackexchange.com/questions/8696/print-this-diamond/100788#100788
        let url = "http://05ab1e.tryitonline.net/#code=OUxKLnDDu3Z5OXlnLcO6w7ss&input=";
        let mut state = LinkState {
            schema: LinkSchema::V1,
            domain: LinkDomain::TryItOnline,
            language: "05ab1e".into(),
            code: "9LJ.pûvy9yg-úû,".into(),
            input: "".into(),
            args: vec![],
            debug: false,
        };
        assert_eq!(state, LinkState::decode_v1(url).unwrap());
        assert_eq!(url, state.encode_v1());
        let url = "https://tio.run/#05ab1e#code=OUxKLnDDu3Z5OXlnLcO6w7ss&input=";
        state.domain = LinkDomain::Tio;
        assert_eq!(state, LinkState::decode_v1(url).unwrap());
        assert_eq!(url, state.encode_v1());

        // A link from 2016-12-09, that was updated 2017-04-06
        // https://codegolf.stackexchange.com/questions/99674/three-polyglots-two-period-two-quines-and-one-code-golf-challenge/102533#102533
        let url = "http://retina.tryitonline.net/#code=VT11bmljaHI7cz1VKDM5KSozO189dScnJ1xuI1U9dW5pY2hyO3M9VSgzOSkqMztfPXUlcy5yZXBsYWNlKFUoOSksVSg5NikpO3ByaW50IF8lJShzK18rcykucmVwbGFjZShVKDEwKSxVKDkyKSsnbicpLnJlcGxhY2UoVSg5NiksVSg5KSkucmVwbGFjZShVKDE3OCksVSgxNzkpKS5yZXBsYWNlKFUoMTgzKSxVKDE4NCkpLnJlcGxhY2UoVSgxODIpLFUoMTgzKSkjfMK2I8K3cHJpbnQiV3JvbmcgbGFuZ3VhZ2UhIsK3Iz8uKnR8IsK3wrYjezJ9fF4uwrZcbiMxCSNcblxuI1QJwrItwrkJX28JW17CuV1cbm49Y2hyKDEwKTtwcmludCBuK24uam9pbihbJ3ByaW50Ildyb25nIGxhbmd1YWdlISInLCcjPy4qdHwiJ10pK25cbicnJy5yZXBsYWNlKFUoOSksVSg5NikpO3ByaW50IF8lKHMrXytzKS5yZXBsYWNlKFUoMTApLFUoOTIpKyduJykucmVwbGFjZShVKDk2KSxVKDkpKS5yZXBsYWNlKFUoMTc4KSxVKDE3OSkpLnJlcGxhY2UoVSgxODMpLFUoMTg0KSkucmVwbGFjZShVKDE4MiksVSgxODMpKSN8CiPCtnByaW50Ildyb25nIGxhbmd1YWdlISLCtiM_Lip0fCLCtgojezJ9fF4uCg&input=";
        let mut state = LinkState {
            schema: LinkSchema::V1,
            domain: LinkDomain::TryItOnline,
            language: "retina".into(),
            code: "U=unichr;s=U(39)*3;_=u'''\\n#U=unichr;s=U(39)*3;_=u%s.replace(U(9),U(96));print _%%(s+_+s).replace(U(10),U(92)+'n').replace(U(96),U(9)).replace(U(178),U(179)).replace(U(183),U(184)).replace(U(182),U(183))#|¶#·print\"Wrong language!\"·#?.*t|\"·¶#{2}|^.¶\\n#1\t#\\n\\n#T\t²-¹\t_o\t[^¹]\\nn=chr(10);print n+n.join(['print\"Wrong language!\"','#?.*t|\"'])+n\\n'''.replace(U(9),U(96));print _%(s+_+s).replace(U(10),U(92)+'n').replace(U(96),U(9)).replace(U(178),U(179)).replace(U(183),U(184)).replace(U(182),U(183))#|\n#¶print\"Wrong language!\"¶#?.*t|\"¶\n#{2}|^.\n".into(),
            input: "".into(),
            args: vec![],
            debug: false,
        };
        assert_eq!(state, LinkState::decode_v1(url).unwrap());
        assert_eq!(url, state.encode_v1());
        let url = "https://tio.run/nexus/retina#code=VT11bmljaHI7cz1VKDM5KSozO189dScnJ1xuI1U9dW5pY2hyO3M9VSgzOSkqMztfPXUlcy5yZXBsYWNlKFUoOSksVSg5NikpO3ByaW50IF8lJShzK18rcykucmVwbGFjZShVKDEwKSxVKDkyKSsnbicpLnJlcGxhY2UoVSg5NiksVSg5KSkucmVwbGFjZShVKDE3OCksVSgxNzkpKS5yZXBsYWNlKFUoMTgzKSxVKDE4NCkpLnJlcGxhY2UoVSgxODIpLFUoMTgzKSkjfMK2I8K3cHJpbnQiV3JvbmcgbGFuZ3VhZ2UhIsK3Iz8uKnR8IsK3wrYjezJ9fF4uwrZcbiMxCSNcblxuI1QJwrItwrkJX28JW17CuV1cbm49Y2hyKDEwKTtwcmludCBuK24uam9pbihbJ3ByaW50Ildyb25nIGxhbmd1YWdlISInLCcjPy4qdHwiJ10pK25cbicnJy5yZXBsYWNlKFUoOSksVSg5NikpO3ByaW50IF8lKHMrXytzKS5yZXBsYWNlKFUoMTApLFUoOTIpKyduJykucmVwbGFjZShVKDk2KSxVKDkpKS5yZXBsYWNlKFUoMTc4KSxVKDE3OSkpLnJlcGxhY2UoVSgxODMpLFUoMTg0KSkucmVwbGFjZShVKDE4MiksVSgxODMpKSN8CiPCtnByaW50Ildyb25nIGxhbmd1YWdlISLCtiM_Lip0fCLCtgojezJ9fF4uCg&input=";
        state.domain = LinkDomain::TioNexus;
        assert_eq!(state, LinkState::decode_v1(url).unwrap());
        assert_eq!(url, state.encode_v1());
    }

    #[test]
    fn base64_plus() {
        // These three links from the same post inexplicably use `+` in base64
        // https://codegolf.stackexchange.com/questions/107944/output-a-program-that-outputs-a-program-that-outputs-ppcg/108195#108195

        // Links from 2017-01-26
        let url = "http://befunge-98.tryitonline.net/#code=ckA7IkBfLCM6PiInIiJBMWpAIiciOjonJ1wiQF8sIzo+IiciIlwnJzo6IiciOicnXCJQUENHIiciIlwnJzo6IiciOicnXCIwQCNqMSInIjo6JydcIj46IyxfQCInIiI7QDtyIic+ayxAPjsjQGshazE&input=";
        let state = LinkState {
            schema: LinkSchema::V1,
            domain: LinkDomain::TryItOnline,
            language: "befunge-98".into(),
            code: "r@;\"@_,#:>\"'\"\"A1j@\"'\"::''\\\"@_,#:>\"'\"\"\\''::\"'\":''\\\"PPCG\"'\"\"\\''::\"'\":''\\\"0@#j1\"'\"::''\\\">:#,_@\"'\"\";@;r\"'>k,@>;#@k!k1".into(),
            input: "".into(),
            args: vec![],
            debug: false,
        };
        assert_eq!(state, LinkState::decode_v1(url).unwrap());
        let url = "http://befunge.tryitonline.net/#code=MWojQDAiR0NQUCI+OiMsX0A&input=";
        let state = LinkState {
            schema: LinkSchema::V1,
            domain: LinkDomain::TryItOnline,
            language: "befunge".into(),
            code: "1j#@0\"GCPP\">:#,_@".into(),
            input: "".into(),
            args: vec![],
            debug: false,
        };
        assert_eq!(state, LinkState::decode_v1(url).unwrap());

        // Link from 2018-01-13
        let url = "http://befunge-96-mtfi.tryitonline.net/#code=QTFqQCJAXywjOj4iJyIiUFBDRyInIiIwQCNqMSI+OiMsX0A&input=";
        let state = LinkState {
            schema: LinkSchema::V1,
            domain: LinkDomain::TryItOnline,
            language: "befunge-96-mtfi".into(),
            code: "A1j@\"@_,#:>\"'\"\"PPCG\"'\"\"0@#j1\">:#,_@".into(),
            input: "".into(),
            args: vec![],
            debug: false,
        };
        assert_eq!(state, LinkState::decode_v1(url).unwrap());
    }

    #[test]
    fn roundtrip_all() {
        let links = include_str!("../../tests/tio_links.txt");
        for link in links.lines() {
            let state = LinkState::decode_v1(link).unwrap();
            let encoded = state.encode_v1();
            if encoded != link {
                // Language-only links like http://cubically.tryitonline.net/
                // encode with empty fragment fields
                if state.domain == LinkDomain::TryItOnline
                    && Url::parse(link).unwrap().fragment().is_none()
                {
                    continue;
                }
                if link.replace("+", "-") == encoded {
                    println!("Differ by `+`: {link} encodes to {encoded}");
                    continue;
                }
                assert_eq!(encoded, link, "encoding {link}");
            }
        }
    }
}
