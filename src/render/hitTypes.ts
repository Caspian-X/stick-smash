// Display info for hitbox types: shared by the debug overlay and the hitbox editor.

import type { HitboxType } from '../data/types';

export interface HitTypeInfo { label: string; icon: string; color: number; css: string; desc: string }

const info = (label: string, icon: string, color: number, desc: string): HitTypeInfo =>
  ({ label, icon, color, css: '#' + color.toString(16).padStart(6, '0'), desc });

export const HIT_TYPES: Record<HitboxType, HitTypeInfo> = {
  normal: info('Regular', '●', 0xff3355, 'Launches at the set angle. Knockback grows with the victim\'s %.'),
  spike: info('Spike', '⤓', 0xff8a1f, 'Meteor: always launches downward. Grounded victims bounce up off the floor.'),
  trip: info('Trip', '⤳', 0x2ad4c0, 'Knocks grounded victims prone (getup options). Airborne victims are hit normally.'),
  sakurai: info('Sakurai angle', '∠', 0xff5dc8, 'Automatic angle: flat on weak grounded hits, higher on strong ones, 45° in the air.'),
  autolink: info('Autolink', '⇶', 0x3fb6ff, 'Drags the victim toward the box center with the attacker\'s momentum. Use with multi-hit.'),
  windbox: info('Windbox', '≋', 0xa8c4ff, 'No damage or hitstun: pushes anything it touches every frame (strength = base KB).'),
  electric: info('Electric', 'ϟ', 0xffe14a, 'Like regular, but the victim stays frozen in hitlag longer, with sparks.'),
  grab: info('Grab', '✋', 0xb070ff, 'Grabs instead of hitting.'),
};

export const HIT_TYPE_ORDER: HitboxType[] = ['normal', 'spike', 'trip', 'sakurai', 'autolink', 'windbox', 'electric', 'grab'];

export const typeOf = (b: { type?: HitboxType }): HitboxType => b.type ?? 'normal';
