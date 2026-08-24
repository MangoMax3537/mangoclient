package gg.mangoclient.mangoconfig.mixin;

import gg.mangoclient.mangoconfig.Presence;
import net.minecraft.client.network.AbstractClientPlayerEntity;
import net.minecraft.client.render.VertexConsumerProvider;
import net.minecraft.client.render.entity.EntityRenderer;
import net.minecraft.client.util.math.MatrixStack;
import net.minecraft.entity.Entity;
import net.minecraft.text.Text;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyVariable;

/**
 * The grey mango over a player's head, for versions before the render-state
 * rewrite (pre 1.21.2): the label text is an argument here, so it is swapped
 * on its way in rather than in any cached state.
 */
@Mixin(EntityRenderer.class)
public abstract class EntityRendererMixin {

	@ModifyVariable(method = "renderLabelIfPresent", at = @At("HEAD"), ordinal = 0, argsOnly = true)
	private Text mangoconfig$badge(Text text, Entity entity, Text textArg, MatrixStack matrices,
			VertexConsumerProvider vertexConsumers, int light, float tickDelta) {
		if (text != null
			&& entity instanceof AbstractClientPlayerEntity player
			&& Presence.badgeEnabled()
			&& Presence.isMango(player.getUuid())) {
			return Presence.withBadge(text, player.getUuid());
		}
		return text;
	}
}
