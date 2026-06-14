// Theia frontend contribution for the Autonomyx language.
// Registers the .ayx file association, syntax highlighting, and
// the LSP client that connects to the backend language server.

import { ContainerModule }         from '@theia/core/shared/inversify';
import { LanguageGrammarDefinitionContribution } from '@theia/monaco/lib/browser/textmate';

const AUTONOMYX_LANGUAGE_ID = 'autonomyx';
const AYX_EXTENSION         = '.ayx';

export default new ContainerModule(bind => {
    bind(LanguageGrammarDefinitionContribution).toConstantValue({
        // Monaco / TextMate language registration
        registerTextmateLanguage(registry: any) {
            registry.registerLanguage({
                id:         AUTONOMYX_LANGUAGE_ID,
                extensions: [AYX_EXTENSION],
                aliases:    ['Autonomyx', 'ayx'],
                mimetypes:  ['text/x-autonomyx'],
                configuration: './language-configuration.json',
            });
            registry.registerGrammar({
                language:  AUTONOMYX_LANGUAGE_ID,
                scopeName: 'source.autonomyx',
                path:      './autonomyx.tmLanguage.json',
            });
        },
    });
});
