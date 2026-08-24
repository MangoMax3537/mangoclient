package gg.mangoclient.mangoconfig.mixin;

import gg.mangoclient.mangoconfig.Presence;
import net.minecraft.client.network.AbstractClientPlayerEntity;
import net.minecraft.client.render.entity.EntityRenderer;
import net.minecraft.client.render.entity.state.EntityRenderState;
import net.minecraft.entity.Entity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * The grey mango over a player's head: the nametag draws whatever ends up in
 * the render state's displayName, and vanilla rebuilds that from the entity
 * every frame just before this runs - so the badge never stacks.
 */
@Mixin(EntityRenderer.class)
public abstract class EntityRendererMixin {

	@Inject(method = "updateRenderState", at = @At("TAIL"))
	private void mangoconfig$badge(Entity entity, EntityRenderState state, float tickDelta, CallbackInfo ci) {
		if (state.displayName != null
			&& entity instanceof AbstractClientPlayerEntity player
			&& Presence.badgeEnabled()
			&& Presence.isMango(player.getUuid())) {
			state.displayName = Presence.withBadge(state.displayName, player.getUuid());
		}
	}
}
