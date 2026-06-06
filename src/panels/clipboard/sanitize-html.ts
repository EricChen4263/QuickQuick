import DOMPurify from "dompurify";

/**
 * 清洗富文本 HTML 后再交给 React dangerouslySetInnerHTML 渲染。
 * 设计 §五 XSS 红线：富文本无信任来源豁免，sanitize 必在入 DOM 前，
 * 由渲染层负责剥离 script / 事件属性 / javascript: 协议等危险内容；
 * 后端原样保存未清洗 HTML 以保真，清洗只发生在此处。
 */
export function sanitizeRichHtml(html: string): string {
  return DOMPurify.sanitize(html);
}
