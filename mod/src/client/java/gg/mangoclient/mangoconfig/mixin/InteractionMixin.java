package gg.mangoclient.mangoconfig.mixin;

import gg.mangoclient.mangoconfig.Combat;
import net.minecraft.client.network.ClientPlayerInteractionManager;
import net.minecraft.entity.Entity;
import net.minecraft.entity.player.PlayerEntity;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * The moment a hit lands, which is the one thing the HUD hook cannot see.
 *
 * Nothing is changed here - the swing goes out exactly as it always did - the
 * hit is only measured on the way past, for the reach and combo modules.
 */
@Mixin(ClientPlayerInteractionManager.class)
public class InteractionMixin {
	@Inject(method = "attackEntity", at = @At("HEAD"))
	private void mangoconfig$onAttack(PlayerEntity player, Entity target, CallbackInfo ci) {
		Combat.onAttack(player, target);
	}
}
