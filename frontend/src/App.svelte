<script lang="ts">
  import { onMount } from "svelte";

  interface StatusResponse {
    status: string;
    version: string;
  }

  interface GreetingResponse {
    message: string;
  }

  let status = $state("loading...");
  let version = $state("");
  let greeting = $state("");

  onMount(async () => {
    try {
      const res = await fetch("/api/status");
      if (!res.ok) throw new Error(`status ${res.status}`);
      const data = (await res.json()) as Partial<StatusResponse>;
      status = data.status ?? "unreachable";
      version = data.version ?? "";
    } catch {
      status = "unreachable";
    }

    try {
      const res = await fetch("/api/greeting");
      if (!res.ok) throw new Error(`status ${res.status}`);
      const data = (await res.json()) as Partial<GreetingResponse>;
      greeting = data.message ?? "Could not reach API";
    } catch {
      greeting = "Could not reach API";
    }
  });
</script>

<main>
  <h1>rustbase</h1>
  <p class="subtitle">Your app is running.</p>

  <div class="card">
    <h2>API Status</h2>
    <dl>
      <dt>Status</dt>
      <dd class:ready={status === "ready"}>{status}</dd>
      <dt>Version</dt>
      <dd>{version || "—"}</dd>
      <dt>Greeting</dt>
      <dd>{greeting || "—"}</dd>
    </dl>
  </div>

  <div class="card">
    <h2>Getting Started</h2>
    <ol>
      <li>Edit <code>frontend/src/App.svelte</code></li>
      <li>
        Add API routes in
        <code>crates/rustbase-web/src/api/mod.rs</code>
      </li>
      <li>
        Run <code>npm run dev</code> in
        <code>frontend/</code> for hot reload
      </li>
    </ol>
  </div>
</main>

<style>
  main {
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
  }

  h1 {
    font-size: 2rem;
    font-weight: 700;
  }

  .subtitle {
    color: var(--muted);
  }

  .card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 1.5rem;
  }

  .card h2 {
    font-size: 1.1rem;
    margin-bottom: 0.75rem;
    color: var(--accent);
  }

  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 0.25rem 1rem;
  }

  dt {
    color: var(--muted);
    font-weight: 500;
  }

  .ready {
    color: #4ade80;
  }

  ol {
    padding-left: 1.25rem;
  }

  li {
    margin-bottom: 0.25rem;
  }
</style>
