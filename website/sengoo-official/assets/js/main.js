const translations = {
  zh: {
    "brand.tagline": "现代系统语言实验室",
    "nav.menu": "菜单",
    "nav.why": "为什么选择",
    "nav.learn": "语言速览",
    "nav.tools": "工具链",
    "nav.runtime": "运行时",
    "nav.getStarted": "开始使用",
    "hero.eyebrow": "编译型 · 异步 · 泛型 · 可集成",
    "hero.title": "为可靠原生软件而生的语言工作台。",
    "hero.lede": "Sengoo 借鉴 Rust 的工程可信度与 Python 的上手友好度：清晰语法、不断成长的工具链、异步运行时原语，以及面向真实系统的原生集成能力。",
    "hero.primary": "立即开始",
    "hero.secondary": "查看语言速览",
    "hero.stat1Label": "当前重点",
    "hero.stat1Value": "Async + Generics",
    "hero.stat2Label": "工具链",
    "hero.stat3Label": "运行时",
    "hero.stat3Value": "原生异步 + 反射",
    "why.eyebrow": "为什么选择 Sengoo?",
    "why.title": "写起来轻盈，编译器内部足够硬核。",
    "why.card1Title": "可预测的原生执行",
    "why.card1Body": "HIR 到 MIR 的降低流程为静态检查、异步帧生成、原生代码生成和定向优化留下了清晰空间。",
    "why.card2Title": "异步路径优先",
    "why.card2Body": "sleep、timeout、spawn、join、select、任务状态与取消正在由编译器和运行时共同打通。",
    "why.card3Title": "持续扩展的泛型标准库",
    "why.card3Body": "Option、Result、collections、math、string 与 error 模块拆分清晰，适合直接在源码层组合。",
    "learn.eyebrow": "语言速览",
    "learn.title": "清爽语法，严肃编译管线。",
    "learn.body": "Sengoo 希望代码能被快速阅读，同时保留能解释错误、优化原生输出并守住异步边界的编译器架构。",
    "tools.eyebrow": "工具链",
    "tools.title": "围绕编译器生长的一组工具。",
    "tools.sgc": "编译器 CLI，覆盖 check、build、run、原生输出与运行时集成。",
    "tools.sgpm": "离线包管理器 MVP，支持 manifest、path dependencies、tree 与 clean。",
    "tools.sgfmt": "格式化入口，随着语言演进保持源码一致性。",
    "tools.sglsp": "语言服务器路径，为编辑器诊断和项目体验打底。",
    "runtime.eyebrow": "运行时集成",
    "runtime.title": "把原生服务包装成 Sengoo 代码能使用的能力。",
    "runtime.body": "运行时栈暴露异步调度、网络句柄、数据库调用、Lua 5.4 桥接、protobuf 辅助函数和 FFI wrapper，让源码示例逐步走向真实集成。",
    "runtime.item1": "异步运行时",
    "runtime.item2": "数据库包装",
    "runtime.item3": "Lua54 桥接",
    "runtime.item4": "Protobuf",
    "runtime.item5": "网络 API",
    "runtime.item6": "原生 FFI",
    "install.eyebrow": "开始使用",
    "install.title": "Clone, build, run.",
    "install.body": "Sengoo 仍在快速演进。先从仓库构建工具链，再用 sgpm 创建或运行一个包。",
    "install.github": "github.com/Hyper66666/Sengoo →",
    "footer.body": "为编译器工程和真实系统软件保留想象力。",
    "footer.github": "源代码 · GitHub",
    "footer.back": "回到顶部"
  },
  en: {
    "brand.tagline": "modern systems language lab",
    "nav.menu": "Menu",
    "nav.why": "Why Sengoo",
    "nav.learn": "Learn",
    "nav.tools": "Tools",
    "nav.runtime": "Runtime",
    "nav.getStarted": "Get Started",
    "hero.eyebrow": "Compiled · Async · Generic · Integrable",
    "hero.title": "A language workbench for reliable native software.",
    "hero.lede": "Sengoo pairs Rust-like engineering confidence with Python-like approachability: clear syntax, a growing toolchain, async runtime primitives, and native integration for real systems.",
    "hero.primary": "Start building",
    "hero.secondary": "Read the tour",
    "hero.stat1Label": "Focus",
    "hero.stat1Value": "Async + Generics",
    "hero.stat2Label": "Toolchain",
    "hero.stat3Label": "Runtime",
    "hero.stat3Value": "Native async + reflection",
    "why.eyebrow": "Why Sengoo?",
    "why.title": "Light to write, serious inside the compiler.",
    "why.card1Title": "Predictable native execution",
    "why.card1Body": "The HIR-to-MIR lowering pipeline leaves clear room for static checks, async frame generation, native codegen, and targeted optimization.",
    "why.card2Title": "Async-first paths",
    "why.card2Body": "sleep, timeout, spawn, join, select, task status, and cancellation are being connected through compiler and runtime together.",
    "why.card3Title": "A growing generic standard library",
    "why.card3Body": "Option, Result, collections, math, string, and error modules are split cleanly for source-level composition.",
    "learn.eyebrow": "Language tour",
    "learn.title": "Readable syntax, rigorous compiler pipeline.",
    "learn.body": "Sengoo aims for code that is quick to scan while preserving a compiler architecture that explains errors, optimizes native output, and protects async boundaries.",
    "tools.eyebrow": "Toolchain",
    "tools.title": "Tools growing around the compiler.",
    "tools.sgc": "Compiler CLI for check, build, run, native output, and runtime integration.",
    "tools.sgpm": "Offline package manager MVP with manifests, path dependencies, tree, and clean.",
    "tools.sgfmt": "Formatter entry point for keeping source files consistent as the language evolves.",
    "tools.sglsp": "Language server path for editor diagnostics and project ergonomics.",
    "runtime.eyebrow": "Runtime integration",
    "runtime.title": "Native services wrapped for Sengoo code.",
    "runtime.body": "The runtime stack exposes async scheduling, network handles, database calls, Lua 5.4 bridges, protobuf helpers, and FFI wrappers so source examples can grow into real integrations.",
    "runtime.item1": "async runtime",
    "runtime.item2": "database wrappers",
    "runtime.item3": "Lua54 bridge",
    "runtime.item4": "Protobuf",
    "runtime.item5": "network APIs",
    "runtime.item6": "native FFI",
    "install.eyebrow": "Get started",
    "install.title": "Clone, build, run.",
    "install.body": "Sengoo is moving quickly. Build the toolchain from the repository, then create or run a package with sgpm.",
    "install.github": "github.com/Hyper66666/Sengoo →",
    "footer.body": "Keeping imagination alive in compiler engineering and real systems software.",
    "footer.github": "Source · GitHub",
    "footer.back": "Back to top"
  }
};

const toggle = document.querySelector(".nav-toggle");
const nav = document.querySelector("#site-nav");
const languageButtons = document.querySelectorAll(".lang-btn");
const i18nNodes = document.querySelectorAll("[data-i18n]");

function setLanguage(lang) {
  const table = translations[lang] || translations.zh;
  document.documentElement.lang = lang === "zh" ? "zh-CN" : "en";
  document.title = lang === "zh" ? "Sengoo 编程语言" : "Sengoo Programming Language";

  for (const node of i18nNodes) {
    const key = node.getAttribute("data-i18n");
    if (key && table[key]) {
      node.textContent = table[key];
    }
  }

  for (const button of languageButtons) {
    const active = button.dataset.lang === lang;
    button.classList.toggle("active", active);
    button.setAttribute("aria-pressed", String(active));
  }

  try {
    localStorage.setItem("sengoo-language", lang);
  } catch (_) {
    // ignore quota / privacy-mode errors
  }
}

if (toggle && nav) {
  toggle.addEventListener("click", () => {
    const isOpen = nav.classList.toggle("is-open");
    toggle.setAttribute("aria-expanded", String(isOpen));
  });

  nav.addEventListener("click", (event) => {
    if (event.target instanceof HTMLAnchorElement) {
      nav.classList.remove("is-open");
      toggle.setAttribute("aria-expanded", "false");
    }
  });
}

for (const button of languageButtons) {
  button.addEventListener("click", () => {
    setLanguage(button.dataset.lang || "zh");
  });
}

let savedLang = "zh";
try {
  savedLang = localStorage.getItem("sengoo-language") || "zh";
} catch (_) {
  // ignore
}
setLanguage(savedLang);

const revealNodes = document.querySelectorAll(".reveal");

if ("IntersectionObserver" in window && revealNodes.length > 0) {
  const observer = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (entry.isIntersecting) {
          entry.target.classList.add("is-visible");
          observer.unobserve(entry.target);
        }
      }
    },
    { rootMargin: "0px 0px -10% 0px", threshold: 0.08 }
  );

  for (const node of revealNodes) {
    observer.observe(node);
  }
} else {
  for (const node of revealNodes) {
    node.classList.add("is-visible");
  }
}
