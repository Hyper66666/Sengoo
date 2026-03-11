import * as fs from 'fs';
import * as path from 'path';

function executableName(toolName: string, platform = process.platform): string {
    return platform === 'win32' ? `${toolName}.exe` : toolName;
}

function dependencyExecutablePattern(toolName: string, platform = process.platform): RegExp {
    const suffix = platform === 'win32' ? '\\.exe' : '';
    return new RegExp(`^${toolName}-[^\\\\/]+${suffix}$`, 'i');
}

function newestExistingFile(paths: string[]): string | undefined {
    let newestPath: string | undefined;
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
        } catch {
            continue;
        }
    }

    return newestPath;
}

function findBundledToolInDeps(root: string, profile: 'debug' | 'release', toolName: string, platform = process.platform): string | undefined {
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

export function uniquePathKey(p: string, platform = process.platform): string {
    return platform === 'win32' ? p.toLowerCase() : p;
}

export function dedupeRoots(roots: Array<string | undefined>, platform = process.platform): string[] {
    const result: string[] = [];
    const seen = new Set<string>();

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

export function findBundledToolUnderRoot(root: string, toolName: string, platform = process.platform): string | undefined {
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

    return (
        findBundledToolInDeps(root, 'debug', toolName, platform) ??
        findBundledToolInDeps(root, 'release', toolName, platform)
    );
}

export function resolveBundledToolPath(roots: Array<string | undefined>, toolName: string, platform = process.platform): string | undefined {
    for (const root of dedupeRoots(roots, platform)) {
        const found = findBundledToolUnderRoot(root, toolName, platform);
        if (found) {
            return found;
        }
    }
    return undefined;
}

export function isLikelyPath(value: string, platform = process.platform): boolean {
    return value.includes('\\') || value.includes('/') || value.endsWith('.exe') || (platform !== 'win32' && value.startsWith('.')) || value.startsWith('.');
}

export function resolveConfiguredToolPath(configured: string, workspaceRoot?: string, filePath?: string): string | undefined {
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
