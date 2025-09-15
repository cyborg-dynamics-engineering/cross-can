///
/// can.rs
///
/// Provides an abstracted CanFrame data struct for use across operating systems.
///
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanFrame {
    id: u32,
    data: [u8; 8],
    dlc: usize,
    is_extended: bool,
    is_rtr: bool,
    is_error: bool,
    timestamp: Option<u64>,
}

impl CanFrame {
    pub fn new(id: u32, data: &[u8]) -> Result<Self, &'static str> {
        Self::validate_id(id, false)?;
        Self::validate_data(data)?;
        let mut buf = [0u8; 8];
        buf[..data.len()].copy_from_slice(data);
        Ok(Self {
            id,
            data: buf,
            dlc: data.len(),
            is_extended: false,
            is_rtr: false,
            is_error: false,
            timestamp: None,
        })
    }

    pub fn new_eff(id: u32, data: &[u8]) -> Result<Self, &'static str> {
        Self::validate_id(id, true)?;
        Self::validate_data(data)?;
        let mut buf = [0u8; 8];
        buf[..data.len()].copy_from_slice(data);
        Ok(Self {
            id,
            data: buf,
            dlc: data.len(),
            is_extended: true,
            is_rtr: false,
            is_error: false,
            timestamp: None,
        })
    }

    pub fn new_remote(id: u32, dlc: usize, is_extended: bool) -> Result<Self, &'static str> {
        if dlc > 8 {
            return Err("RTR frame DLC must be <= 8");
        }
        Self::validate_id(id, is_extended)?;
        Ok(Self {
            id,
            data: [0u8; 8],
            dlc,
            is_extended,
            is_rtr: true,
            is_error: false,
            timestamp: None,
        })
    }

    pub fn new_error(id: u32) -> Result<Self, &'static str> {
        if id > 0x1FFFFFFF {
            return Err("CAN error frame ID must be <= 29 bits");
        }
        Ok(Self {
            id,
            data: [0u8; 8],
            dlc: 0,
            is_extended: false,
            is_rtr: false,
            is_error: true,
            timestamp: None,
        })
    }

    pub fn set_timestamp(&mut self, ts: Option<u64>) {
        self.timestamp = ts;
    }

    pub fn timestamp(&self) -> Option<u64> {
        self.timestamp
    }

    fn validate_id(id: u32, extended: bool) -> Result<(), &'static str> {
        if extended {
            if id > 0x1FFFFFFF {
                return Err("Extended ID must be <= 29 bits (0x1FFFFFFF)");
            }
        } else {
            if id > 0x7FF {
                return Err("Standard ID must be <= 11 bits (0x7FF)");
            }
        }
        Ok(())
    }

    fn validate_data(data: &[u8]) -> Result<(), &'static str> {
        if data.len() > 8 {
            Err("CAN data must be <= 8 bytes")
        } else {
            Ok(())
        }
    }

    pub fn id(&self) -> u32 {
        self.id
    }
    pub fn data(&self) -> &[u8] {
        &self.data[..self.dlc]
    }
    pub fn dlc(&self) -> usize {
        self.dlc
    }
    pub fn is_extended(&self) -> bool {
        self.is_extended
    }
    pub fn is_rtr(&self) -> bool {
        self.is_rtr
    }
    pub fn is_error(&self) -> bool {
        self.is_error
    }
}

#[cfg(target_os = "linux")]
impl From<socketcan::CanFrame> for CanFrame {
    fn from(sc: socketcan::CanFrame) -> Self {
        use socketcan::{self, EmbeddedFrame, Frame};

        let id_raw = match sc.id() {
            socketcan::Id::Standard(standard_id) => standard_id.as_raw() as u32,
            socketcan::Id::Extended(extended_id) => extended_id.as_raw(),
        };

        if sc.is_remote_frame() {
            return CanFrame::new_remote(id_raw, sc.data().len(), sc.is_extended()).unwrap();
        }
        if sc.is_error_frame() {
            return CanFrame::new_error(id_raw).unwrap();
        }
        if sc.is_extended() {
            return CanFrame::new_eff(id_raw, sc.data()).unwrap();
        } else {
            return CanFrame::new(id_raw, sc.data()).unwrap();
        }
    }
}

#[cfg(target_os = "linux")]
impl Into<socketcan::CanFrame> for CanFrame {
    fn into(self) -> socketcan::CanFrame {
        use socketcan::{self, EmbeddedFrame};

        let sc_id = if self.is_extended() {
            match socketcan::ExtendedId::new(self.id()) {
                Some(ext_id) => Ok(socketcan::Id::Extended(ext_id)),
                None => Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Invalid CAN ID for extended can frame: {:?}", self.id()),
                )),
            }
        } else {
            match socketcan::StandardId::new(self.id() as u16) {
                Some(std_id) => Ok(socketcan::Id::Standard(std_id)),
                None => Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Invalid CAN ID for standard can frame: {:?}", self.id()),
                )),
            }
        }
        .unwrap();

        if self.is_error() {
            return socketcan::CanFrame::Error(
                socketcan::CanErrorFrame::new_error(self.id(), self.data()).unwrap(),
            );
        }
        if self.is_rtr() {
            return socketcan::CanFrame::Remote(
                socketcan::CanRemoteFrame::new(sc_id, self.data()).unwrap(),
            );
        }

        socketcan::CanFrame::Data(socketcan::CanDataFrame::new(sc_id, self.data()).unwrap())
    }
}
