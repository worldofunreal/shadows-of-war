import { afterEach, test } from "bun:test";
import assert from "node:assert/strict";
import { SowAuthClient } from "./identity-auth.ts";

const originals = new Map(["window", "localStorage", "fetch"].map(key => [key, Object.getOwnPropertyDescriptor(globalThis, key)]));
const clients: SowAuthClient[] = [];
function environment(handler: (url: string, init?: RequestInit) => Promise<Response>) {
    const values = new Map<string, string>();
    const states: string[] = [];
    Object.defineProperty(globalThis, "localStorage", { configurable: true, value: {
        getItem: (key: string) => values.get(key) ?? null,
        setItem: (key: string, value: string) => values.set(key, value),
        removeItem: (key: string) => values.delete(key),
    } });
    Object.defineProperty(globalThis, "window", { configurable: true, value: { dispatchEvent: (event: CustomEvent) => { states.push(event.detail?.status); return true; } } });
    Object.defineProperty(globalThis, "fetch", { configurable: true, value: handler });
    const client = new SowAuthClient(); clients.push(client);
    return { client, values, states };
}
function result(number = 1) {
    return { status: "success", session_token: "sow_a_" + String(number).repeat(64), refresh_token: "sow_r_" + String(number).repeat(64), expires_at: Math.floor(Date.now() / 1000) + 900,
        account: { account_id: "0123456789abcdef0123456789abcdef", display_name: "Test account", linked_identities: [{ provider: "google" }] } };
}
afterEach(async () => {
    Object.defineProperty(globalThis, "fetch", { configurable: true, value: async () => new Response(null, { status: 204 }) });
    for (const client of clients.splice(0)) await client.logout();
    for (const [key, descriptor] of originals) {
        if (descriptor) Object.defineProperty(globalThis, key, descriptor); else Reflect.deleteProperty(globalThis, key);
    }
});

test("owned sessions refresh once for concurrent callers and persist only a public account card", async () => {
    let refreshes = 0;
    const { client, values } = environment(async (url, init) => {
        assert.ok(url.startsWith("/api/auth/"));
        if (url.endsWith("session/refresh")) { refreshes++; assert.equal(JSON.parse(String(init?.body)).refresh_token, result().refresh_token); return Response.json(result(2)); }
        if (url.endsWith("me")) return Response.json(result().account);
        return Response.json(result());
    });
    await client.startAnonymous();
    const [a, b] = await Promise.all([client.ensureSession(true), client.ensureSession(true)]);
    assert.equal(refreshes, 1);
    assert.equal(a.session_token, b.session_token);
    assert.equal(values.get("sow_session_token"), result(2).session_token);
    assert.equal((await client.getMe())?.linked_identities?.length, 1);
    assert.deepEqual(JSON.parse(values.get("sow_user_data")!), { account_id: result().account.account_id, display_name: "Test account" });
});

test("signing out during refresh cannot restore the previous account", async () => {
    let complete!: (response: Response) => void;
    const { client, values } = environment(async url => url.endsWith("session/refresh") ? await new Promise<Response>(resolve => { complete = resolve; }) : new Response(null, { status: 204 }));
    const original = result();
    client.setSession(original.session_token, original.account, original.refresh_token, original.expires_at);
    const refresh = client.ensureSession(true);
    await client.logout();
    complete(Response.json(result(2)));
    await assert.rejects(refresh, /Account changed during refresh/);
    assert.equal(values.has("sow_session_token"), false);
    assert.equal(values.has("sow_refresh_token"), false);
    assert.equal(values.has("sow_user_data"), false);
});

test("a temporary backend failure retains the existing credentials", async () => {
    const { client, values, states } = environment(async () => Response.json({ error: "temporarily unavailable" }, { status: 503 }));
    const original = result();
    client.setSession(original.session_token, original.account, original.refresh_token, original.expires_at);
    await assert.rejects(client.ensureSession(true), /temporarily unavailable/);
    assert.equal(values.get("sow_session_token"), original.session_token);
    assert.equal(values.get("sow_refresh_token"), original.refresh_token);
    assert.ok(states.includes("retrying"));
});

test("existing local guests must prove ownership before receiving a website session", async () => {
    const {client,values} = environment(async (_url,init) => {
        assert.deepEqual(JSON.parse(String(init?.body)),{account_id:"saved-guest",auth_secret:"private-proof"});
        return Response.json(result());
    });
    values.set("sow_account_id","saved-guest");
    values.set("sow_account_secret","private-proof");
    await client.startAnonymous();
    assert.equal(values.get("sow_session_token"),result().session_token);
});

test("a second tab uses the credential already rotated by the first", async () => {
    let refreshes=0;
    const {client} = environment(async () => { refreshes++; return Response.json(result(2)); });
    client.setSession(result().session_token,result().account,result().refresh_token,result().expires_at);
    const second = new SowAuthClient(); clients.push(second);
    await client.ensureSession(true);
    assert.equal((await second.ensureSession(true)).session_token,result(2).session_token);
    assert.equal(refreshes,1);
});

test("previous guests exchange their proof locally without creating a replacement account", async () => {
    let requests=0;
    const {client,values}=environment(async (url,init)=>{
        requests++;
        assert.equal(url,"/api/auth/session/import");
        const body=JSON.parse(String(init?.body));
        assert.equal(body.session_token,"previous-proof");
        assert.equal(body.refresh_token,null);
        assert.match(body.import_id,/^[0-9a-f]{64}$/);
        return Response.json(result());
    });
    values.set("wou_session_token","previous-proof");
    values.set("wou_user_data",JSON.stringify({account_id:result().account.account_id}));
    assert.equal((await client.startAnonymous()).account.account_id,result().account.account_id);
    assert.equal(requests,1);
    assert.equal(values.has("wou_session_token"),false);
    assert.equal(values.has("wou_user_data"),false);
});

test("a sign-out in another tab prevents an in-flight refresh from restoring it", async()=>{
    let complete!:(response:Response)=>void;
    const {client,values}=environment(async(url,init)=>{
        if(url.endsWith("logout")) {assert.equal(init?.keepalive,true);return new Response(null,{status:204});}
        return await new Promise<Response>(resolve=>{complete=resolve;});
    });
    client.setSession(result().session_token,result().account,result().refresh_token,result().expires_at);
    const pending=client.ensureSession(true);
    values.clear();
    complete(Response.json(result(2)));
    await assert.rejects(pending,/Account changed during refresh/);
    assert.equal(values.has("sow_session_token"),false);
});
