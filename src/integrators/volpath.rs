// std
use std::sync::Arc;
// pbrt
use core::geometry::{Bounds2i, Point2f, Ray, Vector3f};
use core::integrator::SamplerIntegrator;
use core::interaction::{Interaction, InteractionCommon, MediumInteraction, SurfaceInteraction};
use core::lightdistrib::create_light_sample_distribution;
use core::lightdistrib::LightDistribution;
use core::pbrt::{Float, Spectrum};
use core::sampler::Sampler;
use core::scene::Scene;

// see volpath.h

/// Accounts for scattering and attenuation from participating media
/// as well as scattering from surfaces
pub struct VolPathIntegrator {
    // inherited from SamplerIntegrator (see integrator.h)
    pixel_bounds: Bounds2i,
    // see volpath.h
    pub max_depth: u32,
    rr_threshold: Float,           // 1.0
    light_sample_strategy: String, // "spatial"
    light_distribution: Option<Arc<LightDistribution + Send + Sync>>,
}

impl VolPathIntegrator {
    pub fn new(
        max_depth: u32,
        pixel_bounds: Bounds2i,
        rr_threshold: Float,
        light_sample_strategy: String,
    ) -> Self {
        VolPathIntegrator {
            pixel_bounds: pixel_bounds,
            max_depth: max_depth,
            rr_threshold: rr_threshold,
            light_sample_strategy: light_sample_strategy,
            light_distribution: None,
        }
    }
}

impl SamplerIntegrator for VolPathIntegrator {
    fn preprocess(&mut self, scene: &Scene, _sampler: &mut Box<Sampler + Send + Sync>) {
        self.light_distribution =
            create_light_sample_distribution(self.light_sample_strategy.clone(), scene);
    }
    fn li(
        &self,
        r: &mut Ray,
        scene: &Scene,
        sampler: &mut Box<Sampler + Send + Sync>,
        // arena: &mut Arena,
        _depth: i32,
    ) -> Spectrum {
        // TODO: ProfilePhase p(Prof::SamplerIntegratorLi);
        let mut l: Spectrum = Spectrum::default();
        let mut beta: Spectrum = Spectrum::new(1.0 as Float);
        let mut ray: Ray = Ray {
            o: r.o,
            d: r.d,
            t_max: r.t_max,
            time: r.time,
            differential: r.differential,
            medium: r.medium.clone(),
        };
        let mut specular_bounce: bool = false;
        let mut bounces: u32 = 0_u32;
        // Added after book publication: etaScale tracks the
        // accumulated effect of radiance scaling due to rays passing
        // through refractive boundaries (see the derivation on p. 527
        // of the third edition). We track this value in order to
        // remove it from beta when we apply Russian roulette; this is
        // worthwhile, since it lets us sometimes avoid terminating
        // refracted rays that are about to be refracted back out of a
        // medium and thus have their beta value increased.
        let mut eta_scale: Float = 1.0;
        loop {
            // intersect _ray_ with scene and store intersection in _isect_
            let found_intersection: bool;
            if let Some(mut isect) = scene.intersect(&mut ray) {
                found_intersection = true;
                // sample the participating medium, if present
                let mut mi_opt: Option<MediumInteraction> = None;
                if let Some(ref medium) = ray.medium {
                    let (spectrum, option) = medium.sample(&ray, sampler.as_mut());
                    beta *= spectrum;
                    if let Some(mi) = option {
                        mi_opt = Some(mi);
                    }
                }
                if beta.is_black() {
                    break;
                }
            } else {
                found_intersection = false;
            }
        }
        // WORK
        Spectrum::default()
    }
    fn get_pixel_bounds(&self) -> Bounds2i {
        self.pixel_bounds
    }
}
