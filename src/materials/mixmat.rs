//std
use std::sync::Arc;
// pbrt
use crate::core::bssrdf::SeparableBssrdfAdapter;
use crate::core::interaction::SurfaceInteraction;
use crate::core::material::{Material, TransportMode};
use crate::core::microfacet::{
    BeckmannDistribution, MicrofacetDistribution, TrowbridgeReitzDistribution,
};
use crate::core::pbrt::{Float, Spectrum};
use crate::core::reflection::{
    Bsdf, Bxdf, FourierBSDF, Fresnel, FresnelBlend, FresnelConductor, FresnelDielectric,
    FresnelNoOp, FresnelSpecular, LambertianReflection, LambertianTransmission,
    MicrofacetReflection, MicrofacetTransmission, OrenNayar, SpecularReflection,
    SpecularTransmission,
};
use crate::core::texture::Texture;
use crate::materials::disney::{
    DisneyClearCoat, DisneyDiffuse, DisneyFakeSS, DisneyMicrofacetDistribution, DisneyRetro,
    DisneySheen,
};
use crate::materials::hair::HairBSDF;

// see mixmat.h

/// The mix material takes two other materials and a texture and uses
/// the value returned by the texture to blend between the two
/// materials at the point being shaded.
pub struct MixMaterial {
    pub m1: Arc<Material>,
    pub m2: Arc<Material>,
    pub scale: Arc<dyn Texture<Spectrum> + Sync + Send>, // default: 0.5
}

impl MixMaterial {
    pub fn new(
        m1: Arc<Material>,
        m2: Arc<Material>,
        scale: Arc<dyn Texture<Spectrum> + Send + Sync>,
    ) -> Self {
        MixMaterial { m1, m2, scale }
    }
    // Material
    pub fn compute_scattering_functions(
        &self,
        si: &mut SurfaceInteraction,
        arena_bsdf: &mut Vec<Bsdf>,
        arena_bxdf: &mut Vec<Bxdf>,
        mode: TransportMode,
        allow_multiple_lobes: bool,
        _material: Option<Arc<Material>>,
        _scale: Option<Spectrum>,
    ) {
        let s1: Spectrum = self
            .scale
            .evaluate(si)
            .clamp(0.0 as Float, std::f32::INFINITY as Float);
        let s2: Spectrum =
            (Spectrum::new(1.0 as Float) - s1).clamp(0.0 as Float, std::f32::INFINITY as Float);
        let mut si2: SurfaceInteraction = SurfaceInteraction::new(
            &si.common.p,
            &si.common.p_error,
            si.uv,
            &si.common.wo,
            &si.dpdu,
            &si.dpdv,
            &si.dndu,
            &si.dndv,
            si.common.time,
            si.shape,
        );
        self.m1.compute_scattering_functions(
            si,
            arena_bsdf,
            arena_bxdf,
            mode,
            allow_multiple_lobes,
            None,
            Some(s1),
        );
        self.m2.compute_scattering_functions(
            &mut si2,
            arena_bsdf,
            arena_bxdf,
            mode,
            allow_multiple_lobes,
            None,
            Some(s2),
        );
        if let Some(bsdf1_idx) = si.bsdf {
            let bsdf1 = &mut arena_bsdf[bsdf1_idx];
            let n_bxdfs: usize = bsdf1.n_bxdfs as usize;
            // get Bxdfs from si2 before it gets out of scope
            for i2 in 0..n_bxdfs {
                match &arena_bxdf[bsdf1.bxdfs[i2]] {
                    Bxdf::Empty(_bxdf) => break,
                    Bxdf::SpecRefl(bxdf) => {
                        let fresnel = match &bxdf.fresnel {
                            Fresnel::Conductor(fresnel) => Fresnel::Conductor(FresnelConductor {
                                eta_i: fresnel.eta_i,
                                eta_t: fresnel.eta_t,
                                k: fresnel.k,
                            }),
                            Fresnel::Dielectric(fresnel) => {
                                Fresnel::Dielectric(FresnelDielectric {
                                    eta_i: fresnel.eta_i,
                                    eta_t: fresnel.eta_t,
                                })
                            }
                            _ => Fresnel::NoOp(FresnelNoOp {}),
                        };
                        arena_bxdf.push(Bxdf::SpecRefl(SpecularReflection::new(
                            bxdf.r,
                            fresnel,
                            bxdf.sc_opt,
                        )));
                        bsdf1.add(arena_bxdf.len() - 1);
                    }
                    Bxdf::SpecTrans(bxdf) => {
                        arena_bxdf.push(Bxdf::SpecTrans(SpecularTransmission::new(
                            bxdf.t,
                            bxdf.eta_a,
                            bxdf.eta_b,
                            bxdf.mode,
                            bxdf.sc_opt,
                        )));
                        bsdf1.add(arena_bxdf.len() - 1);
                    }
                    Bxdf::FresnelSpec(bxdf) => {
                        arena_bxdf.push(Bxdf::FresnelSpec(FresnelSpecular::new(
                            bxdf.r,
                            bxdf.t,
                            bxdf.eta_a,
                            bxdf.eta_b,
                            bxdf.mode,
                            bxdf.sc_opt,
                        )));
                        bsdf1.add(arena_bxdf.len() - 1);
                    }
                    Bxdf::LambertianRefl(bxdf) => {
                        arena_bxdf.push(Bxdf::LambertianRefl(LambertianReflection::new(
                            bxdf.r,
                            bxdf.sc_opt,
                        )));
                        bsdf1.add(arena_bxdf.len() - 1);
                    }
                    Bxdf::LambertianTrans(bxdf) => {
                        arena_bxdf.push(Bxdf::LambertianTrans(LambertianTransmission::new(
                            bxdf.t,
                            bxdf.sc_opt,
                        )));
                        bsdf1.add(arena_bxdf.len() - 1);
                    }
                    Bxdf::OrenNayarRefl(bxdf) => {
                        arena_bxdf.push(Bxdf::OrenNayarRefl(OrenNayar {
                            r: bxdf.r,
                            a: bxdf.a,
                            b: bxdf.b,
                            sc_opt: bxdf.sc_opt,
                        }));
                        bsdf1.add(arena_bxdf.len() - 1);
                    }
                    Bxdf::MicrofacetRefl(bxdf) => {
                        let distribution = match &bxdf.distribution {
                            MicrofacetDistribution::Beckmann(distribution) => {
                                MicrofacetDistribution::Beckmann(BeckmannDistribution {
                                    alpha_x: distribution.alpha_x,
                                    alpha_y: distribution.alpha_y,
                                    sample_visible_area: distribution.sample_visible_area,
                                })
                            }
                            MicrofacetDistribution::TrowbridgeReitz(distribution) => {
                                MicrofacetDistribution::TrowbridgeReitz(
                                    TrowbridgeReitzDistribution {
                                        alpha_x: distribution.alpha_x,
                                        alpha_y: distribution.alpha_y,
                                        sample_visible_area: distribution.sample_visible_area,
                                    },
                                )
                            }
                            MicrofacetDistribution::DisneyMicrofacet(distribution) => {
                                MicrofacetDistribution::DisneyMicrofacet(
                                    DisneyMicrofacetDistribution::new(
                                        distribution.inner.alpha_x,
                                        distribution.inner.alpha_y,
                                    ),
                                )
                            }
                        };
                        let fresnel = match &bxdf.fresnel {
                            Fresnel::Conductor(fresnel) => Fresnel::Conductor(FresnelConductor {
                                eta_i: fresnel.eta_i,
                                eta_t: fresnel.eta_t,
                                k: fresnel.k,
                            }),
                            Fresnel::Dielectric(fresnel) => {
                                Fresnel::Dielectric(FresnelDielectric {
                                    eta_i: fresnel.eta_i,
                                    eta_t: fresnel.eta_t,
                                })
                            }
                            _ => Fresnel::NoOp(FresnelNoOp {}),
                        };
                        arena_bxdf.push(Bxdf::MicrofacetRefl(MicrofacetReflection::new(
                            bxdf.r,
                            distribution,
                            fresnel,
                            bxdf.sc_opt,
                        )));
                        bsdf1.add(arena_bxdf.len() - 1);
                    }
                    Bxdf::MicrofacetTrans(bxdf) => {
                        let distribution = match &bxdf.distribution {
                            MicrofacetDistribution::Beckmann(distribution) => {
                                MicrofacetDistribution::Beckmann(BeckmannDistribution {
                                    alpha_x: distribution.alpha_x,
                                    alpha_y: distribution.alpha_y,
                                    sample_visible_area: distribution.sample_visible_area,
                                })
                            }
                            MicrofacetDistribution::TrowbridgeReitz(distribution) => {
                                MicrofacetDistribution::TrowbridgeReitz(
                                    TrowbridgeReitzDistribution {
                                        alpha_x: distribution.alpha_x,
                                        alpha_y: distribution.alpha_y,
                                        sample_visible_area: distribution.sample_visible_area,
                                    },
                                )
                            }
                            MicrofacetDistribution::DisneyMicrofacet(distribution) => {
                                MicrofacetDistribution::DisneyMicrofacet(
                                    DisneyMicrofacetDistribution::new(
                                        distribution.inner.alpha_x,
                                        distribution.inner.alpha_y,
                                    ),
                                )
                            }
                        };
                        arena_bxdf.push(Bxdf::MicrofacetTrans(MicrofacetTransmission::new(
                            bxdf.t,
                            distribution,
                            bxdf.eta_a,
                            bxdf.eta_b,
                            bxdf.mode,
                            bxdf.sc_opt,
                        )));
                        bsdf1.add(arena_bxdf.len() - 1);
                    }
                    Bxdf::FresnelBlnd(bxdf) => {
                        let mut distrib: Option<MicrofacetDistribution> = None;
                        if let Some(distribution) = &bxdf.distribution {
                            distrib = match &distribution {
                                MicrofacetDistribution::Beckmann(distribution) => {
                                    Some(MicrofacetDistribution::Beckmann(BeckmannDistribution {
                                        alpha_x: distribution.alpha_x,
                                        alpha_y: distribution.alpha_y,
                                        sample_visible_area: distribution.sample_visible_area,
                                    }))
                                }
                                MicrofacetDistribution::TrowbridgeReitz(distribution) => {
                                    Some(MicrofacetDistribution::TrowbridgeReitz(
                                        TrowbridgeReitzDistribution {
                                            alpha_x: distribution.alpha_x,
                                            alpha_y: distribution.alpha_y,
                                            sample_visible_area: distribution.sample_visible_area,
                                        },
                                    ))
                                }
                                MicrofacetDistribution::DisneyMicrofacet(distribution) => {
                                    Some(MicrofacetDistribution::DisneyMicrofacet(
                                        DisneyMicrofacetDistribution::new(
                                            distribution.inner.alpha_x,
                                            distribution.inner.alpha_y,
                                        ),
                                    ))
                                }
                            }
                        }
                        arena_bxdf.push(Bxdf::FresnelBlnd(FresnelBlend::new(
                            bxdf.rd,
                            bxdf.rs,
                            distrib,
                            bxdf.sc_opt,
                        )));
                        bsdf1.add(arena_bxdf.len() - 1);
                    }
                    Bxdf::Fourier(bxdf) => {
                        arena_bxdf.push(Bxdf::Fourier(FourierBSDF::new(
                            bxdf.bsdf_table.clone(),
                            bxdf.mode,
                            bxdf.sc_opt,
                        )));
                        bsdf1.add(arena_bxdf.len() - 1);
                    }
                    Bxdf::Bssrdf(bxdf) => {
                        arena_bxdf.push(Bxdf::Bssrdf(SeparableBssrdfAdapter {
                            bssrdf: bxdf.bssrdf.clone(),
                            mode: bxdf.mode,
                            eta2: bxdf.eta2,
                        }));
                        bsdf1.add(arena_bxdf.len() - 1);
                    }
                    Bxdf::DisDiff(bxdf) => {
                        arena_bxdf.push(Bxdf::DisDiff(DisneyDiffuse::new(bxdf.r, bxdf.sc_opt)));
                        bsdf1.add(arena_bxdf.len() - 1);
                    }
                    Bxdf::DisSS(bxdf) => {
                        arena_bxdf.push(Bxdf::DisSS(DisneyFakeSS::new(
                            bxdf.r,
                            bxdf.roughness,
                            bxdf.sc_opt,
                        )));
                        bsdf1.add(arena_bxdf.len() - 1);
                    }
                    Bxdf::DisRetro(bxdf) => {
                        arena_bxdf.push(Bxdf::DisRetro(DisneyRetro::new(
                            bxdf.r,
                            bxdf.roughness,
                            bxdf.sc_opt,
                        )));
                        bsdf1.add(arena_bxdf.len() - 1);
                    }
                    Bxdf::DisSheen(bxdf) => {
                        arena_bxdf.push(Bxdf::DisSheen(DisneySheen::new(bxdf.r, bxdf.sc_opt)));
                        bsdf1.add(arena_bxdf.len() - 1);
                    }
                    Bxdf::DisClearCoat(bxdf) => {
                        arena_bxdf.push(Bxdf::DisClearCoat(DisneyClearCoat::new(
                            bxdf.weight,
                            bxdf.gloss,
                            bxdf.sc_opt,
                        )));
                        bsdf1.add(arena_bxdf.len() - 1);
                    }
                    Bxdf::Hair(bxdf) => {
                        arena_bxdf.push(Bxdf::Hair(HairBSDF {
                            h: bxdf.h,
                            gamma_o: bxdf.gamma_o,
                            eta: bxdf.eta,
                            sigma_a: bxdf.sigma_a,
                            beta_m: bxdf.beta_m,
                            beta_n: bxdf.beta_n,
                            v: bxdf.v,
                            s: bxdf.s,
                            sin_2k_alpha: bxdf.sin_2k_alpha,
                            cos_2k_alpha: bxdf.cos_2k_alpha,
                            sc_opt: bxdf.sc_opt,
                        }));
                        bsdf1.add(arena_bxdf.len() - 1);
                    }
                };
            }
        }
    }
}
