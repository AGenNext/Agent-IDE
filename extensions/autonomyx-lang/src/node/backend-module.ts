// Theia backend contribution — registers the Autonomyx LSP server
// as a managed child process. If Theia is replaced with another IDE shell,
// remove this file; the standalone main.ts continues to work.

import { ContainerModule } from '@theia/core/shared/inversify';
import {
    LanguageServerContribution,
    BaseLanguageServerContribution,
} from '@theia/languages/lib/node';
import { injectable } from '@theia/core/shared/inversify';
import * as path from 'path';

@injectable()
export class AutonomyxLanguageServerContribution
    extends BaseLanguageServerContribution {

    readonly id   = 'autonomyx';
    readonly name = 'Autonomyx Language Server';

    async getStartParameters() {
        const serverPath = path.join(__dirname, '..', '..', 'lib', 'node', 'main.js');
        return {
            command: process.execPath,
            args:    [serverPath, '--stdio'],
        };
    }
}

export default new ContainerModule(bind => {
    bind(LanguageServerContribution)
        .to(AutonomyxLanguageServerContribution)
        .inSingletonScope();
});
