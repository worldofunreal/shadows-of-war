import { spawnSync } from "node:child_process";
import * as path from "node:path";
import * as nodeVm from "node:vm";
import * as ts from "typescript";
import campaignApi from "./sow-campaign.ts";
import campaignViewApi from "./sow-campaign-view.ts";

const bundles = new Map();
const contexts = new WeakMap();
const transpiler = new Bun.Transpiler({ loader: "ts", target: "browser" });

function transpileTypeScript(source: string): string {
    return `var exports = globalThis;\n${transpiler.transformSync(source)}`;
}

let scriptResultSequence = 0;

function isSimpleResultExpression(expression: ts.Expression): boolean {
    if (ts.isParenthesizedExpression(expression)) return isSimpleResultExpression(expression.expression);
    return (
        ts.isIdentifier(expression) ||
        ts.isPropertyAccessExpression(expression) ||
        ts.isObjectLiteralExpression(expression) ||
        ts.isArrayLiteralExpression(expression) ||
        ts.isFunctionExpression(expression) ||
        ts.isArrowFunction(expression) ||
        (ts.isCallExpression(expression) &&
            (ts.isIdentifier(expression.expression) || ts.isPropertyAccessExpression(expression.expression)))
    );
}

function scriptSource(source: string, resultKey: string) {
    try {
        new nodeVm.Script(source);
        return { source, resultKey: null as string | null };
    } catch (error) {
        if (!(error instanceof SyntaxError)) throw error;
        const sourceFile = ts.createSourceFile("test-snippet.ts", source, ts.ScriptTarget.Latest, true);
        const lastStatement = sourceFile.statements.at(-1);
        let capturesResult = false;
        if (
            !sourceFile.parseDiagnostics.length &&
            lastStatement &&
            ts.isExpressionStatement(lastStatement) &&
            isSimpleResultExpression(lastStatement.expression)
        ) {
            const expression = lastStatement.expression;
            source =
                source.slice(0, lastStatement.getStart(sourceFile)) +
                `globalThis[${JSON.stringify(resultKey)}] = (${source.slice(expression.getStart(sourceFile), expression.end)});` +
                source.slice(lastStatement.end);
            capturesResult = true;
        }
        return {
            source: transpileTypeScript(source),
            resultKey: capturesResult ? resultKey : null,
        };
    }
}

function rememberGlobals(state, before) {
    for (const key of Object.keys(state.raw)) {
        if (before.get(key) !== state.raw[key]) {
            state.globals.add(key);
            if (typeof state.raw[key] === "function") {
                state.functions.add(key);
                state.wrappers.delete(key);
            }
        }
    }
}

function readGlobal(state, key) {
    const expression = /^[A-Za-z_$][\w$]*$/.test(key) ? key : `globalThis[${JSON.stringify(key)}]`;
    return nodeVm.runInContext(expression, state.raw);
}

function invokeInContext(state, fn, args) {
    const callId = ++state.callId;
    const fnKey = `__sowVmFn${callId}`;
    const argsKey = `__sowVmArgs${callId}`;
    const resultKey = `__sowVmResult${callId}`;
    state.raw[fnKey] = fn;
    state.raw[argsKey] = args;
    try {
        nodeVm.runInContext(
            `globalThis[${JSON.stringify(resultKey)}] = globalThis[${JSON.stringify(fnKey)}].apply(globalThis, globalThis[${JSON.stringify(argsKey)}]);`,
            state.raw,
        );
        return state.raw[resultKey];
    } finally {
        delete state.raw[fnKey];
        delete state.raw[argsKey];
        delete state.raw[resultKey];
    }
}

function wrapFunction(state, fn) {
    return (...args) => invokeInContext(state, fn, args);
}

function trackContext(raw) {
    const state = {
        raw,
        globals: new Set(),
        functions: new Set(),
        wrappers: new Map(),
        callId: 0,
        proxy: null,
    };
    const proxy = new Proxy(raw, {
        get(target, property, receiver) {
            const value = Reflect.get(target, property, receiver);
            if (
                typeof property === "string" &&
                state.functions.has(property) &&
                typeof value === "function"
            ) {
                if (!state.wrappers.has(property)) state.wrappers.set(property, wrapFunction(state, value));
                return state.wrappers.get(property);
            }
            if (typeof property === "string" && state.globals.has(property))
                return readGlobal(state, property);
            return value;
        },
        set(target, property, value, receiver) {
            if (typeof property === "string") {
                state.functions.delete(property);
                state.wrappers.delete(property);
            }
            const assigned = Reflect.set(target, property, value, receiver);
            if (assigned && typeof property === "string" && state.globals.has(property)) {
                const valueKey = `__sowVmAssigned${++state.callId}`;
                target[valueKey] = value;
                try {
                    nodeVm.runInContext(
                        `${/^[A-Za-z_$][\w$]*$/.test(property) ? property : `globalThis[${JSON.stringify(property)}]`} = globalThis[${JSON.stringify(valueKey)}]`,
                        target,
                    );
                } finally {
                    delete target[valueKey];
                }
            }
            return assigned;
        },
    });
    state.proxy = proxy;
    contexts.set(raw, state);
    contexts.set(proxy, state);
    return state;
}

function unwrapContext(context) {
    return contexts.get(context)?.raw || context;
}

function contextSnapshot(raw) {
    return new Map(Object.keys(raw).map((key) => [key, raw[key]]));
}

function runInContext(source, context, options) {
    const raw = unwrapContext(context);
    const state = contexts.get(context) || contexts.get(raw);
    const before = state ? contextSnapshot(raw) : null;
    const resultKey = `__sowVmResult${++scriptResultSequence}`;
    const compiled = scriptSource(source, resultKey);
    const scriptResult = nodeVm.runInContext(compiled.source, raw, options);
    const result = compiled.resultKey ? raw[compiled.resultKey] : scriptResult;
    if (compiled.resultKey) delete raw[compiled.resultKey];
    if (state) rememberGlobals(state, before);
    return result;
}

function runInNewContext(source, sandbox = {}, options) {
    const raw = unwrapContext(sandbox);
    const context = contexts.has(raw)
        ? raw
        : nodeVm.createContext(
              raw,
              options?.contextCodeGeneration ? { codeGeneration: options.contextCodeGeneration } : undefined,
          );
    const state = contexts.get(context) || trackContext(context);
    const before = contextSnapshot(context);
    const resultKey = `__sowVmResult${++scriptResultSequence}`;
    const compiled = scriptSource(source, resultKey);
    const scriptResult = nodeVm.runInContext(compiled.source, context, options);
    const result = compiled.resultKey ? context[compiled.resultKey] : scriptResult;
    if (compiled.resultKey) delete context[compiled.resultKey];
    rememberGlobals(state, before);
    return typeof result === "function" ? wrapFunction(state, result) : result;
}

function createContext(...args) {
    const raw = nodeVm.createContext(...args);
    const state = trackContext(raw);
    return state.proxy;
}

const vm = new Proxy(nodeVm, {
    get(target, property) {
        if (property === "createContext") return createContext;
        if (property === "runInContext") return runInContext;
        if (property === "runInNewContext") return runInNewContext;
        if (property === "Script") {
            return class TypeScriptScript extends nodeVm.Script {
                resultKey: string | null;

                constructor(source, options) {
                    const resultKey = `__sowVmScriptResult${++scriptResultSequence}`;
                    const compiled = scriptSource(source, resultKey);
                    super(compiled.source, options);
                    this.resultKey = compiled.resultKey;
                }

                runInContext(context, options) {
                    const raw = unwrapContext(context);
                    const state = contexts.get(context) || contexts.get(raw);
                    const before = state ? contextSnapshot(raw) : null;
                    const scriptResult = super.runInContext(raw, options);
                    const result = this.resultKey ? raw[this.resultKey] : scriptResult;
                    if (this.resultKey) delete raw[this.resultKey];
                    if (state) rememberGlobals(state, before);
                    return result;
                }
            };
        }
        const value = Reflect.get(target, property, target);
        return typeof value === "function" ? value.bind(target) : value;
    },
});

function bundleTs(file: string): string {
    if (bundles.has(file)) return bundles.get(file);
    const result = spawnSync("bun", ["build", "--target=browser", "--format=iife", file], {
        encoding: "utf8",
        maxBuffer: 16 * 1024 * 1024,
    });
    if (result.error) throw result.error;
    if (result.status !== 0)
        throw new Error(result.stderr || `Bun exited ${result.status} while compiling ${file}`);
    bundles.set(file, result.stdout);
    return result.stdout;
}

const testModules = new Map([
    [path.resolve(import.meta.dir, "sow-campaign.ts"), campaignApi],
    [path.resolve(import.meta.dir, "sow-campaign-view.ts"), campaignViewApi],
]);

function loadTypeScriptModule(file: string) {
    const module = testModules.get(path.resolve(file));
    if (!module) throw new Error(`Unsupported TypeScript test module: ${file}`);
    return module;
}

export { bundleTs, loadTypeScriptModule, transpileTypeScript, vm };
