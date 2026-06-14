// Autonomyx Language Server — standalone LSP process.
// Connects over stdio; works with Theia, VS Code, Neovim, Emacs,
// or any editor that speaks LSP 3.17.
//
// Launch:  node lib/node/main.js --stdio
//
// Zero external runtime dependencies beyond langium and vscode-languageserver.
// No CDN calls, no telemetry, no network access at startup.

import { startLanguageServer } from 'langium/lsp';
import { NodeFileSystem }      from 'langium/node';
import { createConnection, ProposedFeatures } from 'vscode-languageserver/node.js';
import { createAutonomyxServices } from '../language/autonomyx-module.js';

// LSP connection over stdio (safe: no port exposure, no auth surface)
const connection = createConnection(ProposedFeatures.all);

// Boot the language server
const { shared } = createAutonomyxServices({
    connection,
    ...NodeFileSystem,
});

startLanguageServer(shared);
