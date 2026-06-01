import { useState } from "react";

interface GeneralSettings {
  launchOnLogin: boolean;
  stayInTray: boolean;
  autoUpdate: boolean;
  setLaunchOnLogin: (v: boolean) => void;
  setStayInTray: (v: boolean) => void;
  setAutoUpdate: (v: boolean) => void;
}

/**
 * 通用设置本地状态 hook。
 * 里程碑2：useState 本地管理，初始值均为 true（符合开箱即用约定）。
 * 里程碑3：替换为 IPC 读写（read_setting / write_setting），组件侧接口不变。
 */
export function useGeneralSettings(): GeneralSettings {
  const [launchOnLogin, setLaunchOnLogin] = useState(true);
  const [stayInTray, setStayInTray] = useState(true);
  const [autoUpdate, setAutoUpdate] = useState(true);

  return {
    launchOnLogin,
    stayInTray,
    autoUpdate,
    setLaunchOnLogin,
    setStayInTray,
    setAutoUpdate,
  };
}
