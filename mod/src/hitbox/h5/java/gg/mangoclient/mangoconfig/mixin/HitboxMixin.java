package gg.mangoclient.mangoconfig.mixin;

import gg.mangoclient.mangoconfig.Visuals;
import net.minecraft.client.render.debug.EntityHitboxDebugRenderer;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyArg;

/**
 * Hitboxes, 1.21.11+: drawn as gizmos - every box through DrawStyle.stroked
 * and the look vector as an arrow, both with an ARGB colour.
 */
@Mixin(EntityHitboxDebugRenderer.class)
public class HitboxMixin {
	@ModifyArg(method = "drawHitbox", at = @At(value = "INVOKE",
		target = "Lnet/minecraft/client/render/DrawStyle;stroked(I)Lnet/minecraft/client/render/DrawStyle;"))
	private int mangoconfig$box(int colour) {
		return Visuals.hitbox(colour);
	}

	@ModifyArg(method = "drawHitbox", at = @At(value = "INVOKE",
		target = "Lnet/minecraft/world/debug/gizmo/GizmoDrawing;arrow(Lnet/minecraft/util/math/Vec3d;"
			+ "Lnet/minecraft/util/math/Vec3d;I)Lnet/minecraft/world/debug/gizmo/VisibilityConfigurable;"),
		index = 2)
	private int mangoconfig$look(int colour) {
		return Visuals.hitbox(colour);
	}
}
