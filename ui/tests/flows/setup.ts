/**
 * Development environment constants and setup notes.
 *
 * The `pnpm start` command (via mprocs) launches two Tauri agents:
 *   - Agent 1: port 9223 (default MCP Bridge port)
 *   - Agent 2: port 9224
 *
 * Connect to each agent via:
 *   driver_session({ action: 'start', port: AGENT_1_PORT })
 *   driver_session({ action: 'start', port: AGENT_2_PORT })
 */

export const AGENT_1_PORT = 9223;
export const AGENT_2_PORT = 9224;
