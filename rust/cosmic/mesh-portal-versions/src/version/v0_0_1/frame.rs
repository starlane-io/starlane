pub mod frame {
    use core::str::FromStr;
    use std::convert::TryInto;
    use nom::AsBytes;
    use semver::Version;

    use serde::{Deserialize, Serialize};

    use crate::error::MsgErr;
    use crate::version::v0_0_1::sys::EntryReq;
    use crate::version::v0_0_1::wave::{ReqShell, RespShell, Wave};

    pub struct PrimitiveFrame {
        pub data: Vec<u8>,
    }

    impl PrimitiveFrame {
        pub fn size(&self) -> u32 {
            self.data.len() as u32
        }
    }

    impl From<Vec<u8>> for PrimitiveFrame {
        fn from(value: Vec<u8>) -> Self {
            Self { data: value }
        }
    }

    impl From<String> for PrimitiveFrame {
        fn from(value: String) -> Self {
            let bytes = value.as_bytes();
            Self {
                data: bytes.to_vec(),
            }
        }
    }

    impl TryInto<String> for PrimitiveFrame {
        type Error = MsgErr;

        fn try_into(self) -> Result<String, Self::Error> {
            Ok(String::from_utf8(self.data)?)
        }
    }

    impl TryInto<semver::Version> for PrimitiveFrame {
        type Error = MsgErr;

        fn try_into(self) -> Result<semver::Version, Self::Error> {
            let data = String::from_utf8(self.data)?;
            Ok(semver::Version::from_str(data.as_str() )? )
        }
    }

    impl From<semver::Version> for PrimitiveFrame  {
        fn from(version: Version) -> Self {
            let data = version.to_string();
            PrimitiveFrame::from(data)
        }
    }


    #[derive(Debug, Clone, Serialize, Deserialize, strum_macros::Display)]
    pub enum CloseReason {
        Done,
        Error(String),
    }


    impl TryInto<PrimitiveFrame> for ReqShell{
        type Error = MsgErr;

        fn try_into(self) -> Result<PrimitiveFrame, Self::Error> {
            let data = bincode::serialize(&self)?;
            Ok(PrimitiveFrame::from(data))
        }
    }

    impl TryInto<ReqShell> for PrimitiveFrame {
        type Error = MsgErr;

        fn try_into(self) -> Result<ReqShell, Self::Error> {
            Ok(bincode::deserialize(self.data.as_bytes())?)
        }
    }


    impl TryInto<PrimitiveFrame> for RespShell{
        type Error = MsgErr;

        fn try_into(self) -> Result<PrimitiveFrame, Self::Error> {
            let data = bincode::serialize(&self)?;
            Ok(PrimitiveFrame::from(data))
        }
    }

    impl TryInto<RespShell> for PrimitiveFrame {
        type Error = MsgErr;

        fn try_into(self) -> Result<RespShell, Self::Error> {
            Ok(bincode::deserialize(self.data.as_bytes())?)
        }
    }


    impl TryInto<PrimitiveFrame> for Wave{
        type Error = MsgErr;

        fn try_into(self) -> Result<PrimitiveFrame, Self::Error> {
            let data = bincode::serialize(&self)?;
            Ok(PrimitiveFrame::from(data))
        }
    }

    impl TryInto<Wave> for PrimitiveFrame {
        type Error = MsgErr;

        fn try_into(self) -> Result<Wave, Self::Error> {
            Ok(bincode::deserialize(self.data.as_bytes())?)
        }
    }


    impl TryInto<PrimitiveFrame> for EntryReq{
        type Error = MsgErr;

        fn try_into(self) -> Result<PrimitiveFrame, Self::Error> {
            let data = bincode::serialize(&self)?;
            Ok(PrimitiveFrame::from(data))
        }
    }

    impl TryInto<EntryReq> for PrimitiveFrame {
        type Error = MsgErr;

        fn try_into(self) -> Result<EntryReq, Self::Error> {
            Ok(bincode::deserialize(self.data.as_bytes())?)
        }
    }

}
