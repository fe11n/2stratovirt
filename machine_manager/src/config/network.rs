// Copyright (c) 2020 Huawei Technologies Co.,Ltd. All rights reserved.
//
// StratoVirt is licensed under Mulan PSL v2.
// You can use this software according to the terms and conditions of the Mulan
// PSL v2.
// You may obtain a copy of Mulan PSL v2 at:
//         http://license.coscl.org.cn/MulanPSL2
// THIS SOFTWARE IS PROVIDED ON AN "AS IS" BASIS, WITHOUT WARRANTIES OF ANY
// KIND, EITHER EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO
// NON-INFRINGEMENT, MERCHANTABILITY OR FIT FOR A PARTICULAR PURPOSE.
// See the Mulan PSL v2 for more details.

extern crate serde;
extern crate serde_json;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::errors::{ErrorKind, Result};
use crate::config::{CmdParser, ConfigCheck, ExBool, VmConfig};

const MAX_STRING_LENGTH: usize = 255;
const MAC_ADDRESS_LENGTH: usize = 17;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetDevcfg {
    pub id: String,
    pub mac: Option<String>,
    pub tap_fd: Option<i32>,
    pub vhost_type: Option<String>,
    pub vhost_fd: Option<i32>,
}

impl Default for NetDevcfg {
    fn default() -> Self {
        NetDevcfg {
            id: "".to_string(),
            mac: None,
            tap_fd: None,
            vhost_type: None,
            vhost_fd: None,
        }
    }
}

/// Config struct for network
/// Contains network device config, such as `host_dev_name`, `mac`...
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkInterfaceConfig {
    pub id: String,
    pub host_dev_name: String,
    pub mac: Option<String>,
    pub tap_fd: Option<i32>,
    pub vhost_type: Option<String>,
    pub vhost_fd: Option<i32>,
    pub iothread: Option<String>,
}

impl NetworkInterfaceConfig {
    pub fn set_mac(&mut self, mac_addr: String) {
        self.mac = Some(mac_addr);
    }
}

impl Default for NetworkInterfaceConfig {
    fn default() -> Self {
        NetworkInterfaceConfig {
            id: "".to_string(),
            host_dev_name: "".to_string(),
            mac: None,
            tap_fd: None,
            vhost_type: None,
            vhost_fd: None,
            iothread: None,
        }
    }
}

impl ConfigCheck for NetworkInterfaceConfig {
    fn check(&self) -> Result<()> {
        if self.id.len() > MAX_STRING_LENGTH {
            return Err(ErrorKind::StringLengthTooLong("id".to_string(), MAX_STRING_LENGTH).into());
        }

        if self.host_dev_name.len() > MAX_STRING_LENGTH {
            return Err(ErrorKind::StringLengthTooLong(
                self.host_dev_name.clone(),
                MAX_STRING_LENGTH,
            )
            .into());
        }

        if self.mac.is_some() && !check_mac_address(self.mac.as_ref().unwrap()) {
            return Err(ErrorKind::MacFormatError.into());
        }

        if let Some(vhost_type) = self.vhost_type.as_ref() {
            if vhost_type != "vhost-kernel" {
                return Err(ErrorKind::UnknownVhostType.into());
            }
        }

        if self.iothread.is_some() && self.iothread.as_ref().unwrap().len() > MAX_STRING_LENGTH {
            return Err(ErrorKind::StringLengthTooLong(
                "iothread name".to_string(),
                MAX_STRING_LENGTH,
            )
            .into());
        }

        Ok(())
    }
}

pub fn parse_netdev(cmd_parser: CmdParser) -> Result<NetDevcfg> {
    let mut net = NetDevcfg::default();
    if let Some(net_id) = cmd_parser.get_value::<String>("id")? {
        net.id = net_id;
    } else {
        return Err(ErrorKind::FieldIsMissing("id", "net").into());
    }
    if let Some(vhost) = cmd_parser.get_value::<ExBool>("vhost")? {
        if vhost.into() {
            net.vhost_type = Some(String::from("vhost-kernel"));
        }
    }
    net.mac = cmd_parser.get_value::<String>("mac")?;
    net.tap_fd = cmd_parser.get_value::<i32>("fds")?;
    net.vhost_fd = cmd_parser.get_value::<i32>("vhostfds")?;
    Ok(net)
}

pub fn parse_net(vm_config: &VmConfig, net_config: &str) -> Result<NetworkInterfaceConfig> {
    let mut cmd_parser = CmdParser::new("virtio-net-device");
    cmd_parser
        .push("")
        .push("id")
        .push("netdev")
        .push("mac")
        .push("fds")
        .push("vhost")
        .push("vhostfds")
        .push("iothread");

    cmd_parser.parse(net_config)?;

    let mut netdevinterfacecfg = NetworkInterfaceConfig::default();

    let netdev = if let Some(devname) = cmd_parser.get_value::<String>("netdev")? {
        devname
    } else {
        return Err(ErrorKind::FieldIsMissing("netdev", "net").into());
    };
    let netid = if let Some(id) = cmd_parser.get_value::<String>("id")? {
        id
    } else {
        "".to_string()
    };
    netdevinterfacecfg.iothread = cmd_parser.get_value::<String>("iothread")?;

    let netconfig = &vm_config.netdevs;
    if netconfig.is_none() {
        bail!("No netdev configuration for net device found");
    }

    if let Some(netcfg) = netconfig.as_ref().unwrap().get(&netdev) {
        netdevinterfacecfg.id = netid;
        netdevinterfacecfg.host_dev_name = netcfg.id.clone();
        netdevinterfacecfg.mac = netcfg.mac.clone();
        netdevinterfacecfg.tap_fd = netcfg.tap_fd;
        netdevinterfacecfg.vhost_fd = netcfg.vhost_fd;
        netdevinterfacecfg.vhost_type = netcfg.vhost_type.clone();
    } else {
        bail!("Netdev: {:?} not found for net device", &netdev);
    }
    netdevinterfacecfg.check()?;
    Ok(netdevinterfacecfg)
}

impl VmConfig {
    pub fn add_netdev(&mut self, netdev_config: &str) -> Result<()> {
        let mut cmd_parser = CmdParser::new("netdev");
        cmd_parser
            .push("")
            .push("id")
            .push("mac")
            .push("fds")
            .push("vhost")
            .push("vhostfds");

        cmd_parser.parse(netdev_config)?;
        let drive_cfg = parse_netdev(cmd_parser)?;
        if self.netdevs.is_none() {
            self.netdevs = Some(HashMap::new());
        }
        let netdev_id = drive_cfg.id.clone();
        if self.netdevs.as_mut().unwrap().get(&netdev_id).is_none() {
            self.netdevs.as_mut().unwrap().insert(netdev_id, drive_cfg);
        } else {
            bail!("Netdev {:?} has been added");
        }

        Ok(())
    }
}

fn check_mac_address(mac: &str) -> bool {
    if mac.len() != MAC_ADDRESS_LENGTH {
        return false;
    }

    let mac_vec: Vec<&str> = mac.split(':').collect();
    if mac_vec.len() != 6 {
        return false;
    }

    let bit_list = [
        '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f', 'A', 'B',
        'C', 'D', 'E', 'F',
    ];
    for mac_bit in mac_vec {
        if mac_bit.len() != 2 {
            return false;
        }
        let mut mac_bit_char = mac_bit.chars();
        if !bit_list.contains(&mac_bit_char.next().unwrap())
            || !bit_list.contains(&mac_bit_char.next().unwrap())
        {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_config_cmdline_parser() {
        let mut vm_config = VmConfig::default();
        assert!(vm_config.add_netdev("id=eth0").is_ok());
        let net_cfg_res = parse_net(
            &vm_config,
            "virtio-net-device,id=net0,netdev=eth0,iothread=iothread0",
        );
        assert!(net_cfg_res.is_ok());
        let network_configs = net_cfg_res.unwrap();
        assert_eq!(network_configs.id, "net0");
        assert_eq!(network_configs.host_dev_name, "eth0");
        assert_eq!(network_configs.iothread, Some("iothread0".to_string()));
        assert!(network_configs.mac.is_none());
        assert!(network_configs.tap_fd.is_none());
        assert!(network_configs.vhost_type.is_none());
        assert!(network_configs.vhost_fd.is_none());

        let mut vm_config = VmConfig::default();
        assert!(vm_config
            .add_netdev("id=eth1,mac=12:34:56:78:9A:BC,vhost=on,vhostfds=4")
            .is_ok());
        let net_cfg_res = parse_net(&vm_config, "virtio-net-device,id=net1,netdev=eth1");
        assert!(net_cfg_res.is_ok());
        let network_configs = net_cfg_res.unwrap();
        assert_eq!(network_configs.id, "net1");
        assert_eq!(network_configs.host_dev_name, "eth1");
        assert_eq!(network_configs.mac, Some(String::from("12:34:56:78:9A:BC")));
        assert!(network_configs.tap_fd.is_none());
        assert_eq!(
            network_configs.vhost_type,
            Some(String::from("vhost-kernel"))
        );
        assert_eq!(network_configs.vhost_fd, Some(4));

        let mut vm_config = VmConfig::default();
        assert!(vm_config
            .add_netdev("id=eth1,mac=12:34:56:78:9A:BC,vhost=on,vhostfds=4")
            .is_ok());
        let net_cfg_res = parse_net(&vm_config, "virtio-net-device,id=net1,netdev=eth2");
        assert!(net_cfg_res.is_err());
    }
}
