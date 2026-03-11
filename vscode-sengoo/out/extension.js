"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
exports.activate = activate;
exports.deactivate = deactivate;
const vscode = __importStar(require("vscode"));
const path = __importStar(require("path"));
const cp = __importStar(require("child_process"));
const toolPaths_1 = require("./toolPaths");
let client;
let outputChannel;
let warnedInvalidConfiguredSgcPath = false;
let warnedFallbackToPathSgc = false;
let warnedInvalidConfiguredLspPath = false;
let warnedFallbackToPathLsp = false;
function activate(context) {
    outputChannel = vscode.window.createOutputChannel('Sengoo');
    outputChannel.appendLine('Sengoo 扩展已激活');
    const config = vscode.workspace.getConfiguration('sengoo');
    const lspEnabled = config.get('lsp.enabled', true);
    if (lspEnabled) {
        startLspClient(context, config);
    }
    // 注册状态栏
    const statusBar = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 100);
    statusBar.text = '$(play) Sengoo';
    statusBar.tooltip = '点击运行当前 Sengoo 文件';
    statusBar.command = 'sengoo.run';
    context.subscriptions.push(statusBar);
    const updateStatusBar = () => {
        const editor = vscode.window.activeTextEditor;
        if (editor && editor.document.languageId === 'sengoo') {
            statusBar.show();
        }
        else {
            statusBar.hide();
        }
    };
    vscode.window.onDidChangeActiveTextEditor(updateStatusBar, null, context.subscriptions);
    updateStatusBar();
    // 注册命令
    context.subscriptions.push(vscode.commands.registerCommand('sengoo.showInfo', () => {
        vscode.window.showInformationMessage('Sengoo Language v1.0.0 - Python 风格 + Rust 类型系统');
    }), vscode.commands.registerCommand('sengoo.run', () => runSengooFile('run')), vscode.commands.registerCommand('sengoo.build', () => runSengooFile('build')), vscode.commands.registerCommand('sengoo.buildAndRun', () => runSengooFile('buildAndRun')), vscode.commands.registerCommand('sengoo.check', () => runSengooFile('check')));
    // 注册调试配置提供者 (Initial - 处理 F5 无 launch.json 的情况)
    context.subscriptions.push(vscode.debug.registerDebugConfigurationProvider('sengoo', new SengooDebugConfigProviderInitial(), vscode.DebugConfigurationProviderTriggerKind.Initial));
    // 注册调试配置提供者 (Dynamic - 处理"添加配置"下拉)
    context.subscriptions.push(vscode.debug.registerDebugConfigurationProvider('sengoo', new SengooDebugConfigProviderDynamic(), vscode.DebugConfigurationProviderTriggerKind.Dynamic));
    // 注册 resolve 提供者 (填充缺失字段)
    context.subscriptions.push(vscode.debug.registerDebugConfigurationProvider('sengoo', new SengooDebugConfigResolver()));
    // 注册调试适配器工厂
    context.subscriptions.push(vscode.debug.registerDebugAdapterDescriptorFactory('sengoo', new SengooDebugAdapterFactory()));
    // 监听配置变更
    vscode.workspace.onDidChangeConfiguration((e) => {
        if (e.affectsConfiguration('sengoo.lsp')) {
            const newConfig = vscode.workspace.getConfiguration('sengoo');
            const newEnabled = newConfig.get('lsp.enabled', true);
            if (newEnabled && !client) {
                startLspClient(context, newConfig);
            }
            else if (!newEnabled && client) {
                stopLspClient();
            }
        }
    }, null, context.subscriptions);
}
// ========== sgc 命令执行 ==========
function resolveWorkspaceRoot(filePath) {
    if (filePath) {
        const folder = vscode.workspace.getWorkspaceFolder(vscode.Uri.file(filePath));
        if (folder) {
            return folder.uri.fsPath;
        }
    }
    return vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
}
function collectToolSearchRoots(filePath) {
    const roots = [];
    const pushRoot = (candidate) => {
        if (!candidate) {
            return;
        }
        roots.push(candidate);
    };
    pushRoot(resolveWorkspaceRoot(filePath));
    if (filePath) {
        let dir = path.dirname(filePath);
        while (true) {
            pushRoot(dir);
            const parent = path.dirname(dir);
            if (parent === dir) {
                break;
            }
            dir = parent;
        }
    }
    return roots;
}
function resolveConfiguredTool(configured, filePath) {
    return (0, toolPaths_1.resolveConfiguredToolPath)(configured, resolveWorkspaceRoot(filePath), filePath);
}
function resolveBundledTool(toolName, filePath) {
    return (0, toolPaths_1.resolveBundledToolPath)(collectToolSearchRoots(filePath), toolName);
}
function getSgcPath(filePath) {
    const config = vscode.workspace.getConfiguration('sengoo');
    const configured = config.get('sgc.path', '').trim();
    if (configured) {
        const configuredResolved = resolveConfiguredTool(configured, filePath);
        if (configuredResolved) {
            return configuredResolved;
        }
        if (!warnedInvalidConfiguredSgcPath) {
            warnedInvalidConfiguredSgcPath = true;
            vscode.window.showWarningMessage(`Sengoo 配置项 sengoo.sgc.path 无效：${configured}，将回退到自动探测。`);
        }
    }
    const bundled = resolveBundledTool('sgc', filePath);
    if (bundled) {
        return bundled;
    }
    if (!warnedFallbackToPathSgc) {
        warnedFallbackToPathSgc = true;
        vscode.window.showWarningMessage('Sengoo 未在项目内找到 target/debug/sgc，已回退到 PATH 中的 sgc（可能是旧版本）。');
    }
    return 'sgc';
}
function getLspPath(filePath) {
    const config = vscode.workspace.getConfiguration('sengoo');
    const configured = config.get('lsp.path', '').trim();
    if (configured) {
        const configuredResolved = resolveConfiguredTool(configured, filePath);
        if (configuredResolved) {
            return configuredResolved;
        }
        if (!warnedInvalidConfiguredLspPath) {
            warnedInvalidConfiguredLspPath = true;
            vscode.window.showWarningMessage(`Sengoo 配置项 sengoo.lsp.path 无效：${configured}，将回退到自动探测。`);
        }
    }
    const bundled = resolveBundledTool('sglsp', filePath);
    if (bundled) {
        return bundled;
    }
    if (!warnedFallbackToPathLsp) {
        warnedFallbackToPathLsp = true;
        vscode.window.showWarningMessage('Sengoo 未在项目内找到可用的 sglsp，将回退到 PATH 中的 sglsp。');
    }
    return 'sglsp';
}
function getActiveFile() {
    const editor = vscode.window.activeTextEditor;
    if (!editor) {
        vscode.window.showWarningMessage('没有打开的文件');
        return undefined;
    }
    if (editor.document.languageId !== 'sengoo') {
        vscode.window.showWarningMessage('当前文件不是 Sengoo 文件 (.sg)');
        return undefined;
    }
    editor.document.save();
    return editor.document.fileName;
}
function runSengooFile(mode) {
    const filePath = getActiveFile();
    if (!filePath) {
        return;
    }
    const sgc = getSgcPath(filePath);
    outputChannel.appendLine(`[Sengoo] 使用编译器: ${sgc}`);
    const cwd = path.dirname(filePath);
    const terminal = vscode.window.createTerminal({
        name: `Sengoo: ${path.basename(filePath)}`,
        cwd,
    });
    terminal.show();
    // Windows PowerShell 需要 & 调用运算符来执行带引号的命令
    const isWin = process.platform === 'win32';
    const cmd = (exe, ...args) => {
        const quotedArgs = args.map(a => `"${a}"`).join(' ');
        return isWin ? `& "${exe}" ${quotedArgs}` : `"${exe}" ${quotedArgs}`;
    };
    switch (mode) {
        case 'run':
            terminal.sendText(cmd(sgc, 'run', filePath));
            break;
        case 'build':
            terminal.sendText(cmd(sgc, 'build', filePath));
            break;
        case 'buildAndRun': {
            const dir = path.dirname(filePath);
            const stem = path.basename(filePath, '.sg');
            const exe = path.join(dir, 'build', stem + (isWin ? '.exe' : ''));
            terminal.sendText(`${cmd(sgc, 'build', filePath)}; if ($LASTEXITCODE -eq 0) { ${cmd(exe)} }`);
            break;
        }
        case 'check':
            terminal.sendText(cmd(sgc, 'check', filePath));
            break;
    }
}
// ========== 调试配置提供者 ==========
/**
 * Initial provider: 当用户按 F5 且没有 launch.json 时，
 * 提供默认配置列表让用户选择
 */
class SengooDebugConfigProviderInitial {
    provideDebugConfigurations(_folder, _token) {
        return [
            {
                type: 'sengoo',
                request: 'launch',
                name: 'Sengoo: 运行当前文件',
                program: '${file}',
                mode: 'run',
                cwd: '${workspaceFolder}',
            },
            {
                type: 'sengoo',
                request: 'launch',
                name: 'Sengoo: 编译并运行',
                program: '${file}',
                mode: 'build',
                cwd: '${workspaceFolder}',
            },
        ];
    }
}
/**
 * Dynamic provider: 用于"添加配置"下拉菜单
 */
class SengooDebugConfigProviderDynamic {
    provideDebugConfigurations(_folder, _token) {
        return [
            {
                type: 'sengoo',
                request: 'launch',
                name: 'Sengoo: 运行当前文件',
                program: '${file}',
                mode: 'run',
                cwd: '${workspaceFolder}',
            },
            {
                type: 'sengoo',
                request: 'launch',
                name: 'Sengoo: 编译并运行',
                program: '${file}',
                mode: 'build',
                cwd: '${workspaceFolder}',
            },
        ];
    }
}
/**
 * Resolver: 在启动调试前填充/修正配置
 * 这是关键 — 当 F5 时没有 launch.json，VSCode 会传入空配置，
 * 我们需要在这里填充完整配置
 */
class SengooDebugConfigResolver {
    resolveDebugConfiguration(folder, config, _token) {
        // F5 时没有 launch.json: config 是空对象 {}
        if (!config.type && !config.request && !config.name) {
            const editor = vscode.window.activeTextEditor;
            if (editor && editor.document.languageId === 'sengoo') {
                config.type = 'sengoo';
                config.request = 'launch';
                config.name = 'Sengoo: 运行当前文件';
                config.program = editor.document.fileName;
                config.mode = 'run';
                config.cwd = folder?.uri.fsPath || path.dirname(editor.document.fileName);
                config.sgcPath = getSgcPath(editor.document.fileName);
                return config;
            }
            // 不是 .sg 文件，返回 undefined 让 VSCode 走其他调试器
            return undefined;
        }
        // 有 launch.json 配置，填充缺失字段
        if (!config.program) {
            const editor = vscode.window.activeTextEditor;
            if (editor) {
                config.program = editor.document.fileName;
            }
            else {
                vscode.window.showErrorMessage('Sengoo: 找不到要运行的文件，请打开一个 .sg 文件');
                return undefined;
            }
        }
        if (!config.sgcPath) {
            config.sgcPath = getSgcPath(config.program);
        }
        if (!config.cwd) {
            config.cwd = folder?.uri.fsPath || path.dirname(config.program);
        }
        if (!config.mode) {
            config.mode = 'run';
        }
        return config;
    }
}
// ========== 调试适配器 ==========
class SengooDebugAdapterFactory {
    createDebugAdapterDescriptor(_session, _executable) {
        return new vscode.DebugAdapterInlineImplementation(new SengooDebugSession());
    }
}
/**
 * 简易调试会话 - 实现 DAP 协议的最小子集
 * 编译并运行 .sg 文件，在调试控制台显示输出
 */
class SengooDebugSession {
    constructor() {
        this.sendMessage = new vscode.EventEmitter();
        this.onDidSendMessage = this.sendMessage.event;
        this.seq = 1;
    }
    handleMessage(message) {
        const msg = message;
        switch (msg.command) {
            case 'initialize':
                this.sendResponse(msg, {
                    supportsConfigurationDoneRequest: true,
                    supportsTerminateRequest: true,
                });
                this.sendEvent('initialized', {});
                break;
            case 'configurationDone':
                this.sendResponse(msg, {});
                break;
            case 'launch':
                this.sendResponse(msg, {});
                this.doLaunch(msg.arguments);
                break;
            case 'terminate':
                this.killChild();
                this.sendResponse(msg, {});
                this.sendEvent('terminated', {});
                break;
            case 'disconnect':
                this.killChild();
                this.sendResponse(msg, {});
                break;
            case 'threads':
                this.sendResponse(msg, {
                    threads: [{ id: 1, name: 'main' }],
                });
                break;
            default:
                // 对未知请求也返回成功，避免 VSCode 报错
                this.sendResponse(msg, {});
                break;
        }
    }
    doLaunch(args) {
        const program = args.program || '';
        const sgcPath = args.sgcPath || 'sgc';
        const mode = args.mode || 'run';
        const cwd = args.cwd || path.dirname(program);
        const buildArgs = args.buildArgs || [];
        outputChannel.appendLine(`[Sengoo Debug] 使用编译器: ${sgcPath}`);
        this.sendOutputEvent(`🚀 Sengoo: ${mode === 'build' ? '编译并运行' : '运行'} ${path.basename(program)}\n`, 'console');
        if (mode === 'build') {
            this.doBuildAndRun(sgcPath, program, cwd, buildArgs);
        }
        else {
            this.doRun(sgcPath, program, cwd);
        }
    }
    doRun(sgcPath, program, cwd) {
        const spawnArgs = ['run', program];
        this.sendOutputEvent(`> ${sgcPath} ${spawnArgs.join(' ')}\n\n`, 'console');
        this.childProcess = cp.spawn(sgcPath, spawnArgs, {
            cwd,
            shell: true,
            stdio: ['ignore', 'pipe', 'pipe'],
        });
        this.childProcess.stdout?.on('data', (data) => {
            this.sendOutputEvent(data.toString(), 'stdout');
        });
        this.childProcess.stderr?.on('data', (data) => {
            this.sendOutputEvent(data.toString(), 'stderr');
        });
        this.childProcess.on('close', (code) => {
            this.sendOutputEvent(`\n进程退出，返回码: ${code ?? 'unknown'}\n`, 'console');
            this.sendEvent('terminated', {});
        });
        this.childProcess.on('error', (err) => {
            this.sendOutputEvent(`\n❌ 执行失败: ${err.message}\n`, 'stderr');
            this.sendOutputEvent('提示: 请确保 sgc 已安装并在 PATH 中，或在设置中配置 sengoo.sgc.path\n', 'console');
            this.sendEvent('terminated', {});
        });
    }
    doBuildAndRun(sgcPath, program, cwd, buildArgs) {
        const spawnArgs = ['build', program, ...buildArgs];
        this.sendOutputEvent(`> ${sgcPath} ${spawnArgs.join(' ')}\n\n`, 'console');
        const buildProcess = cp.spawn(sgcPath, spawnArgs, {
            cwd,
            shell: true,
            stdio: ['ignore', 'pipe', 'pipe'],
        });
        buildProcess.stdout?.on('data', (data) => {
            this.sendOutputEvent(data.toString(), 'stdout');
        });
        buildProcess.stderr?.on('data', (data) => {
            this.sendOutputEvent(data.toString(), 'stderr');
        });
        buildProcess.on('close', (code) => {
            if (code === 0) {
                const dir = path.dirname(program);
                const stem = path.basename(program, '.sg');
                const exe = path.join(dir, 'build', stem + (process.platform === 'win32' ? '.exe' : ''));
                this.sendOutputEvent(`\n> ${exe}\n\n`, 'console');
                this.childProcess = cp.spawn(exe, [], {
                    cwd,
                    shell: true,
                    stdio: ['ignore', 'pipe', 'pipe'],
                });
                this.childProcess.stdout?.on('data', (data) => {
                    this.sendOutputEvent(data.toString(), 'stdout');
                });
                this.childProcess.stderr?.on('data', (data) => {
                    this.sendOutputEvent(data.toString(), 'stderr');
                });
                this.childProcess.on('close', (runCode) => {
                    this.sendOutputEvent(`\n进程退出，返回码: ${runCode ?? 'unknown'}\n`, 'console');
                    this.sendEvent('terminated', {});
                });
                this.childProcess.on('error', (err) => {
                    this.sendOutputEvent(`\n❌ 运行失败: ${err.message}\n`, 'stderr');
                    this.sendEvent('terminated', {});
                });
            }
            else {
                this.sendOutputEvent(`\n❌ 编译失败，返回码: ${code}\n`, 'stderr');
                this.sendEvent('terminated', {});
            }
        });
        buildProcess.on('error', (err) => {
            this.sendOutputEvent(`\n❌ 编译器执行失败: ${err.message}\n`, 'stderr');
            this.sendOutputEvent('提示: 请确保 sgc 已安装并在 PATH 中，或在设置中配置 sengoo.sgc.path\n', 'console');
            this.sendEvent('terminated', {});
        });
    }
    killChild() {
        if (this.childProcess) {
            this.childProcess.kill();
            this.childProcess = undefined;
        }
    }
    sendResponse(request, body) {
        this.sendMessage.fire({
            seq: this.seq++,
            type: 'response',
            request_seq: request.seq,
            command: request.command,
            success: true,
            body,
        });
    }
    sendEvent(event, body) {
        this.sendMessage.fire({
            seq: this.seq++,
            type: 'event',
            event,
            body,
        });
    }
    sendOutputEvent(output, category) {
        this.sendEvent('output', { category, output });
    }
    dispose() {
        this.killChild();
        this.sendMessage.dispose();
    }
}
// ========== LSP 客户端 ==========
function startLspClient(context, config) {
    let languageClientModule;
    try {
        languageClientModule = require('vscode-languageclient/node');
    }
    catch (err) {
        const msg = `Sengoo LSP 未启动：缺少 vscode-languageclient 依赖（${err?.message ?? err}）`;
        outputChannel.appendLine(msg);
        vscode.window.showWarningMessage(msg);
        return;
    }
    const activeFilePath = vscode.window.activeTextEditor?.document.fileName;
    const command = getLspPath(activeFilePath);
    outputChannel.appendLine(`[Sengoo LSP] Using server: ${command}`);
    const serverOptions = {
        run: { command, transport: languageClientModule.TransportKind.stdio },
        debug: { command, transport: languageClientModule.TransportKind.stdio },
    };
    const clientOptions = {
        documentSelector: [{ scheme: 'file', language: 'sengoo' }],
        synchronize: {
            fileEvents: vscode.workspace.createFileSystemWatcher('**/*.sg'),
        },
        outputChannelName: 'Sengoo LSP',
    };
    client = new languageClientModule.LanguageClient('sengoo-lsp', 'Sengoo Language Server', serverOptions, clientOptions);
    client.start().catch((err) => {
        const message = `Sengoo LSP failed to start: ${err.message}`;
        outputChannel.appendLine(message);
        vscode.window.showWarningMessage(message);
        console.warn(message);
    });
    context.subscriptions.push({
        dispose: () => stopLspClient(),
    });
}
function stopLspClient() {
    if (client) {
        client.stop();
        client = undefined;
    }
}
function deactivate() {
    return client?.stop();
}
//# sourceMappingURL=extension.js.map