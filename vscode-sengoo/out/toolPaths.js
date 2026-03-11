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
exports.uniquePathKey = uniquePathKey;
exports.dedupeRoots = dedupeRoots;
exports.findBundledToolUnderRoot = findBundledToolUnderRoot;
exports.resolveBundledToolPath = resolveBundledToolPath;
exports.isLikelyPath = isLikelyPath;
exports.resolveConfiguredToolPath = resolveConfiguredToolPath;
const fs = __importStar(require("fs"));
const path = __importStar(require("path"));
function executableName(toolName, platform = process.platform) {
    return platform === 'win32' ? `${toolName}.exe` : toolName;
}
function dependencyExecutablePattern(toolName, platform = process.platform) {
    const suffix = platform === 'win32' ? '\\.exe' : '';
    return new RegExp(`^${toolName}-[^\\\\/]+${suffix}$`, 'i');
}
function newestExistingFile(paths) {
    let newestPath;
    let newestMtime = Number.NEGATIVE_INFINITY;
    for (const filePath of paths) {
        try {
            const stat = fs.statSync(filePath);
            if (!stat.isFile()) {
                continue;
            }
            if (stat.mtimeMs > newestMtime) {
                newestMtime = stat.mtimeMs;
                newestPath = filePath;
            }
        }
        catch {
            continue;
        }
    }
    return newestPath;
}
function findBundledToolInDeps(root, profile, toolName, platform = process.platform) {
    const depsDir = path.join(root, 'target', profile, 'deps');
    if (!fs.existsSync(depsDir)) {
        return undefined;
    }
    const pattern = dependencyExecutablePattern(toolName, platform);
    const candidates = fs.readdirSync(depsDir)
        .filter((entry) => pattern.test(entry))
        .map((entry) => path.join(depsDir, entry));
    return newestExistingFile(candidates);
}
function uniquePathKey(p, platform = process.platform) {
    return platform === 'win32' ? p.toLowerCase() : p;
}
function dedupeRoots(roots, platform = process.platform) {
    const result = [];
    const seen = new Set();
    for (const candidate of roots) {
        if (!candidate) {
            continue;
        }
        const normalized = path.normalize(candidate);
        const key = uniquePathKey(normalized, platform);
        if (!seen.has(key)) {
            seen.add(key);
            result.push(normalized);
        }
    }
    return result;
}
function findBundledToolUnderRoot(root, toolName, platform = process.platform) {
    const exe = executableName(toolName, platform);
    const directCandidates = [
        path.join(root, 'target', 'debug', exe),
        path.join(root, 'target', 'release', exe),
    ];
    for (const candidate of directCandidates) {
        if (fs.existsSync(candidate)) {
            return candidate;
        }
    }
    return (findBundledToolInDeps(root, 'debug', toolName, platform) ??
        findBundledToolInDeps(root, 'release', toolName, platform));
}
function resolveBundledToolPath(roots, toolName, platform = process.platform) {
    for (const root of dedupeRoots(roots, platform)) {
        const found = findBundledToolUnderRoot(root, toolName, platform);
        if (found) {
            return found;
        }
    }
    return undefined;
}
function isLikelyPath(value, platform = process.platform) {
    return value.includes('\\') || value.includes('/') || value.endsWith('.exe') || (platform !== 'win32' && value.startsWith('.')) || value.startsWith('.');
}
function resolveConfiguredToolPath(configured, workspaceRoot, filePath) {
    if (!configured) {
        return undefined;
    }
    if (!isLikelyPath(configured)) {
        return configured;
    }
    if (path.isAbsolute(configured)) {
        return fs.existsSync(configured) ? configured : undefined;
    }
    const base = workspaceRoot || (filePath ? path.dirname(filePath) : process.cwd());
    const resolved = path.resolve(base, configured);
    return fs.existsSync(resolved) ? resolved : undefined;
}
//# sourceMappingURL=toolPaths.js.map