package gg.mangoclient.mangoconfig;

import net.minecraft.entity.Entity;
import net.minecraft.entity.player.PlayerEntity;
import net.minecraft.util.math.Box;
import net.minecraft.util.math.Vec3d;

import java.util.UUID;

/**
 * What the reach and combo modules read.
 *
 * Both need to know about a hit the moment it lands, which is the one thing a
 * HUD hook cannot see, so the attack mixin feeds this and the modules only
 * ever read from it.
 */
public final class Combat {
	/** How long a reach reading stays on screen after the hit. */
	private static final long REACH_HOLD_MS = 4000;
	/** A gap longer than this ends a combo. */
	private static final long COMBO_GAP_MS = 3000;

	private static double reach;
	private static long reachAt;

	private static UUID victim;
	private static int combo;
	private static long lastHit;

	private Combat() {
	}

	/** Called from the attack mixin, before the swing is sent. */
	public static void onAttack(PlayerEntity player, Entity target) {
		if (player == null || target == null) return;

		reach = distanceToHitbox(player, target);
		reachAt = System.currentTimeMillis();

		long now = reachAt;
		UUID id = target.getUuid();
		boolean sameFight = id.equals(victim) && now - lastHit <= COMBO_GAP_MS;
		combo = sameFight ? combo + 1 : 1;
		victim = id;
		lastHit = now;
	}

	/**
	 * Distance from the eye to the nearest point of the target's hitbox, which
	 * is what "reach" means in a fight - centre-to-centre would read high on a
	 * wide mob and low on a small one.
	 */
	private static double distanceToHitbox(PlayerEntity player, Entity target) {
		Vec3d eye = player.getEyePos();
		Box box = target.getBoundingBox();
		double dx = Math.max(0, Math.max(box.minX - eye.x, eye.x - box.maxX));
		double dy = Math.max(0, Math.max(box.minY - eye.y, eye.y - box.maxY));
		double dz = Math.max(0, Math.max(box.minZ - eye.z, eye.z - box.maxZ));
		return Math.sqrt(dx * dx + dy * dy + dz * dz);
	}

	public static boolean hasReach() {
		return reachAt != 0 && System.currentTimeMillis() - reachAt <= REACH_HOLD_MS;
	}

	public static double reach() {
		return reach;
	}

	/** 0 when the last hit is too far back to still count as a combo. */
	public static int combo() {
		if (lastHit == 0 || System.currentTimeMillis() - lastHit > COMBO_GAP_MS) return 0;
		return combo;
	}

	/** Seconds left before the combo lapses, for the fading colour. */
	public static float comboFreshness() {
		if (combo() == 0) return 0f;
		long gone = System.currentTimeMillis() - lastHit;
		return 1f - Math.min(1f, gone / (float) COMBO_GAP_MS);
	}
}
