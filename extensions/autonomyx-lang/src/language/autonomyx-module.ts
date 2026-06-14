// Langium DI module for the Autonomyx language.
// This is the single entry point that wires grammar → parser → validator → LSP.
// It is IDE-agnostic: the same module runs in Theia, VS Code, or any
// standalone LSP host process.

import {
    createDefaultModule,
    createDefaultSharedModule,
    DefaultSharedModuleContext,
    inject,
    LangiumServices,
    LangiumSharedServices,
    Module,
    PartialLangiumServices,
} from 'langium';
import { AutonomyxGeneratedModule, AutonomyxGeneratedSharedModule } from './generated/module.js';
import { registerValidationChecks } from './autonomyx-validator.js';

export type AutonomyxAddedServices = {
    // Extend here as the language grows (code generators, formatters, etc.)
};

export type AutonomyxServices = LangiumServices & AutonomyxAddedServices;

export const AutonomyxModule: Module<AutonomyxServices, PartialLangiumServices & AutonomyxAddedServices> = {
    // No overrides yet — defaults are sufficient for Phase 1.
};

export function createAutonomyxServices(context: DefaultSharedModuleContext): {
    shared: LangiumSharedServices;
    Autonomyx: AutonomyxServices;
} {
    const shared = inject(
        createDefaultSharedModule(context),
        AutonomyxGeneratedSharedModule,
    );
    const Autonomyx = inject(
        createDefaultModule({ shared }),
        AutonomyxGeneratedModule,
        AutonomyxModule,
    );
    shared.ServiceRegistry.register(Autonomyx);
    registerValidationChecks(Autonomyx.validation.ValidationRegistry);
    return { shared, Autonomyx };
}
