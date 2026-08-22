package gg.mangoclient.mangoconfig.mixin;

import gg.mangoclient.mangoconfig.Pvp;
import net.minecraft.client.particle.Particle;
import net.minecraft.client.particle.ParticleManager;
import net.minecraft.particle.ParticleEffect;
import net.minecraft.particle.ParticleType;
import net.minecraft.particle.ParticleTypes;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * No Explosions: crystal, TNT and wind charge particles never spawn.
 *
 * Every explosion the client hears about becomes a particle through this one
 * method, so cancelling here catches them all - and returning null is a value
 * vanilla already hands back when a particle is out of range, so every caller
 * copes with it.
 */
@Mixin(ParticleManager.class)
public class ParticleManagerMixin {
	@Inject(
		method = "addParticle(Lnet/minecraft/particle/ParticleEffect;DDDDDD)"
			+ "Lnet/minecraft/client/particle/Particle;",
		at = @At("HEAD"), cancellable = true)
	private void mangoconfig$noExplosions(ParticleEffect parameters, double x, double y, double z,
			double velocityX, double velocityY, double velocityZ, CallbackInfoReturnable<Particle> cir) {
		if (!Pvp.noExplosions.value) return;
		ParticleType<?> type = parameters.getType();
		if (type == ParticleTypes.EXPLOSION
			|| type == ParticleTypes.EXPLOSION_EMITTER
			|| type == ParticleTypes.GUST
			|| type == ParticleTypes.SMALL_GUST
			|| type == ParticleTypes.GUST_EMITTER_LARGE
			|| type == ParticleTypes.GUST_EMITTER_SMALL) {
			cir.setReturnValue(null);
		}
	}
}
