export type SelectEvent =
  | "text_selected"
  | "icon_clicked"
  | "hotkey_translate"
  | "click_elsewhere";

export type SelectAction = "show_icon" | "translate" | "dismiss";

/**
 * 根据用户事件决定触发何种行为。
 *
 * 设计纪律（§八#5）：text_selected 只冒图标，绝不自动翻译；
 * 必须由用户主动点图标或按 Cmd+Shift+T 才触发翻译。
 */
export function resolveSelectAction(event: SelectEvent): SelectAction {
  switch (event) {
    case "text_selected":
      return "show_icon";
    case "icon_clicked":
      return "translate";
    case "hotkey_translate":
      return "translate";
    case "click_elsewhere":
      return "dismiss";
  }
}
