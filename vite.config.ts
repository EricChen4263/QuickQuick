import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { resolve } from "path";

// https://vitejs.dev/config/
export default defineConfig(async () => ({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
  build: {
    rollupOptions: {
      input: {
        main: resolve(__dirname, "index.html"),
        "clip-popover": resolve(__dirname, "src/clip-popover/index.html"),
        "trans-popover": resolve(__dirname, "src/trans-popover/index.html"),
      },
    },
  },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test-setup.ts"],
    environmentOptions: {
      jsdom: {
        url: "http://localhost",
      },
    },
    // 覆盖率只建立基线，不设任何阈值：门禁先不拦，等基线稳定后再谈。
    // 统计范围限定为 src/ 下的源码；测试文件、测试装配与类型声明不计入，
    // 否则「测试代码自己给自己刷分」会让数字失去意义。
    coverage: {
      provider: "v8",
      // 测试失败时也照出报告：覆盖率是诊断信号，不是门禁——失败时更需要知道
      // 「哪块没被测到」来定位问题。默认 false 会让失败运行一份报告都不留。
      reportOnFailure: true,
      reporter: ["text", "json-summary", "html"],
      reportsDirectory: "./coverage",
      include: ["src/**/*.{ts,tsx}"],
      exclude: [
        "src/**/*.test.{ts,tsx}",
        "src/test-setup.ts",
        "src/**/*.d.ts",
      ],
    },
  },
}));
