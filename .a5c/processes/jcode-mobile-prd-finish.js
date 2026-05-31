/**
 * @process local/jcode-mobile-prd-finish
 * @description Finish the JCode Mobile PRD by auditing current implementation, closing the next highest-leverage PRD gaps, updating docs, and verifying Rust/iOS surfaces.
 * @skill mobile-testing specializations/mobile-development/skills/mobile-testing/SKILL.md
 * @agent ios-native-expert specializations/mobile-development/agents/ios-native-expert/AGENT.md
 * @agent mobile-qa-expert specializations/mobile-development/agents/mobile-qa-expert/AGENT.md
 * @inputs { prdPath: string, targetSlice?: string }
 * @outputs { success: boolean, audit: object, implementation: object, verification: object, docs: object, finalReview: object }
 */

import { defineTask } from '@a5c-ai/babysitter-sdk';

export async function process(inputs, ctx) {
  const prdPath = inputs.prdPath || 'docs/JCODE_MOBILE_PRD.md';
  const targetSlice = inputs.targetSlice || 'highest leverage JCode Mobile PRD gap that is bounded and verifiable now';

  ctx.log('info', 'Phase 1: PRD/current-code audit');
  const audit = await ctx.task(auditTask, { prdPath, targetSlice });

  ctx.log('info', 'Phase 2: Implement selected PRD slice');
  const implementation = await ctx.task(implementationTask, { prdPath, targetSlice, audit });

  ctx.log('info', 'Phase 3: Verify mobile Rust/iOS surfaces');
  let verification = await ctx.task(verificationTask, { prdPath, implementation });

  let remediation = null;
  if (!verification.success) {
    ctx.log('warn', 'Verification failed; running remediation loop');
    remediation = await ctx.task(remediationTask, { prdPath, implementation, verification });
    verification = await ctx.task(verificationTask, { prdPath, implementation, remediation });
  }

  ctx.log('info', 'Phase 4: Update PRD and handoff docs');
  const docs = await ctx.task(docsTask, { prdPath, audit, implementation, verification, remediation });

  ctx.log('info', 'Phase 5: Final quality review');
  const finalReview = await ctx.task(finalReviewTask, { prdPath, audit, implementation, verification, docs });

  return {
    success: Boolean(verification.success && finalReview.success),
    audit,
    implementation,
    verification,
    remediation,
    docs,
    finalReview,
  };
}

export const auditTask = defineTask('jcode-mobile-prd-audit', (args, taskCtx) => ({
  kind: 'agent',
  title: 'Audit JCode Mobile PRD against current code',
  agent: {
    name: 'ios-native-expert',
    prompt: {
      role: 'senior Rust/iOS mobile architect',
      task: 'Audit the JCode Mobile PRD against the current repository and choose the next bounded implementation slice.',
      context: args,
      instructions: [
        'Read the PRD and current Rust mobile core, simulator, Swift bridge, and diagnostics surfaces.',
        'Separate already-implemented PRD items from real gaps.',
        'Pick the highest-leverage bounded slice that can be implemented and verified in this workspace now.',
        'Preserve the native Swift host plus shared Rust core direction.',
        'Return concrete file targets and executable verification commands.'
      ],
      outputFormat: 'JSON'
    },
    outputSchema: {
      type: 'object',
      required: ['alreadyDone', 'selectedSlice', 'files', 'verificationCommands'],
      properties: {
        alreadyDone: { type: 'array', items: { type: 'string' } },
        selectedSlice: { type: 'string' },
        files: { type: 'array', items: { type: 'string' } },
        verificationCommands: { type: 'array', items: { type: 'string' } }
      }
    }
  },
  io: {
    inputJsonPath: `tasks/${taskCtx.effectId}/input.json`,
    outputJsonPath: `tasks/${taskCtx.effectId}/output.json`
  },
  labels: ['jcode', 'mobile', 'prd', 'audit']
}));

export const implementationTask = defineTask('jcode-mobile-prd-implement', (args, taskCtx) => ({
  kind: 'agent',
  title: 'Implement selected JCode Mobile PRD slice',
  agent: {
    name: 'ios-native-expert',
    prompt: {
      role: 'principal Rust and Swift mobile engineer',
      task: 'Implement the selected PRD slice directly in the repository.',
      context: args,
      instructions: [
        'Make surgical code changes only in files needed for the selected slice.',
        'Add focused Rust and/or Swift tests where the slice changes shared behavior.',
        'Keep product behavior in jcode-mobile-core; Swift should host platform effects and rendering.',
        'Do not modify unrelated local user changes.',
        'Return files changed, behavior implemented, and any residual risks.'
      ],
      outputFormat: 'JSON'
    },
    outputSchema: {
      type: 'object',
      required: ['success', 'filesChanged', 'summary'],
      properties: {
        success: { type: 'boolean' },
        filesChanged: { type: 'array', items: { type: 'string' } },
        summary: { type: 'string' },
        risks: { type: 'array', items: { type: 'string' } }
      }
    }
  },
  io: {
    inputJsonPath: `tasks/${taskCtx.effectId}/input.json`,
    outputJsonPath: `tasks/${taskCtx.effectId}/output.json`
  },
  labels: ['jcode', 'mobile', 'implementation']
}));

export const verificationTask = defineTask('jcode-mobile-prd-verify', (args, taskCtx) => ({
  kind: 'shell',
  title: 'Run JCode Mobile verification checks',
  shell: {
    command: [
      'cargo test -p jcode-mobile-core',
      'cargo test -p jcode-mobile-sim',
      'cargo check -p jcode-mobile-core -p jcode-mobile-sim',
      'swift run --package-path ios JCodeKitTests',
      'git diff --check'
    ].join(' && '),
    timeout: 900000,
    outputPath: `tasks/${taskCtx.effectId}/output.json`
  },
  io: {
    inputJsonPath: `tasks/${taskCtx.effectId}/input.json`,
    outputJsonPath: `tasks/${taskCtx.effectId}/output.json`
  },
  labels: ['jcode', 'mobile', 'verify']
}));

export const remediationTask = defineTask('jcode-mobile-prd-remediate', (args, taskCtx) => ({
  kind: 'agent',
  title: 'Remediate failed JCode Mobile verification',
  agent: {
    name: 'mobile-qa-expert',
    prompt: {
      role: 'senior mobile QA engineer and debugger',
      task: 'Fix verification failures without reducing PRD scope.',
      context: args,
      instructions: [
        'Inspect the failing command output and root cause.',
        'Patch only the relevant files.',
        'Preserve the selected PRD slice intent.',
        'Return changed files and the expected verification result.'
      ],
      outputFormat: 'JSON'
    },
    outputSchema: {
      type: 'object',
      required: ['success', 'filesChanged', 'summary'],
      properties: {
        success: { type: 'boolean' },
        filesChanged: { type: 'array', items: { type: 'string' } },
        summary: { type: 'string' }
      }
    }
  },
  io: {
    inputJsonPath: `tasks/${taskCtx.effectId}/input.json`,
    outputJsonPath: `tasks/${taskCtx.effectId}/output.json`
  },
  labels: ['jcode', 'mobile', 'remediation']
}));

export const docsTask = defineTask('jcode-mobile-prd-docs', (args, taskCtx) => ({
  kind: 'agent',
  title: 'Update JCode Mobile PRD and handoff notes',
  agent: {
    name: 'generalist',
    prompt: {
      role: 'technical product engineer',
      task: 'Update the PRD and handoff artifacts to match the code that now exists.',
      context: args,
      instructions: [
        'Update PRD status, current state, milestones, next slice, and verification evidence.',
        'Keep wording concrete and distinguish implemented, verified, planned, and blocked items.',
        'Record a concise handoff under .context for other agents.',
        'Do not overclaim physical-device or APNs verification if not actually run.'
      ],
      outputFormat: 'JSON'
    },
    outputSchema: {
      type: 'object',
      required: ['success', 'filesChanged', 'summary'],
      properties: {
        success: { type: 'boolean' },
        filesChanged: { type: 'array', items: { type: 'string' } },
        summary: { type: 'string' }
      }
    }
  },
  io: {
    inputJsonPath: `tasks/${taskCtx.effectId}/input.json`,
    outputJsonPath: `tasks/${taskCtx.effectId}/output.json`
  },
  labels: ['jcode', 'mobile', 'docs']
}));

export const finalReviewTask = defineTask('jcode-mobile-prd-final-review', (args, taskCtx) => ({
  kind: 'agent',
  title: 'Final review against PRD',
  agent: {
    name: 'mobile-qa-expert',
    prompt: {
      role: 'mobile release reviewer',
      task: 'Review the final diff and verification against the PRD.',
      context: args,
      instructions: [
        'Check that implementation matches the PRD slice.',
        'Check that docs do not overstate verification.',
        'Report remaining PRD work and whether the run can be considered complete.'
      ],
      outputFormat: 'JSON'
    },
    outputSchema: {
      type: 'object',
      required: ['success', 'summary', 'remainingWork'],
      properties: {
        success: { type: 'boolean' },
        summary: { type: 'string' },
        remainingWork: { type: 'array', items: { type: 'string' } }
      }
    }
  },
  io: {
    inputJsonPath: `tasks/${taskCtx.effectId}/input.json`,
    outputJsonPath: `tasks/${taskCtx.effectId}/output.json`
  },
  labels: ['jcode', 'mobile', 'review']
}));
