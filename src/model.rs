use alloc::vec::Vec;

use glam::Vec3;
use rand::Rng;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LaserMode {
    #[default]
    Ceiling,
    Random,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct BoundingBox {
    pub min: Vec3,
    pub max: Vec3,
}

impl BoundingBox {
    fn dimensions(&self) -> Vec3 {
        self.max - self.min
    }

    fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }
}

/// A normalised point cloud with per-point rendering attributes.
#[derive(Debug, Clone)]
pub struct PointCloud {
    pub points: Vec<f32>,
    pub attributes: Vec<f32>,
    pub scale: f32,
    pub center: Vec3,
    pub bbox: BoundingBox,
}

impl PointCloud {
    /// Deserialize point cloud from raw binary buffer.
    pub fn from_bytes(buffer: &[u8]) -> Self {
        assert!(
            buffer.len().is_multiple_of(4),
            "Buffer length must be a multiple of 4"
        );

        let mut points = Vec::with_capacity(buffer.len() / 4);

        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);

        for chunk in buffer.chunks_exact(12) {
            let x = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
            let y = f32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
            let z = f32::from_le_bytes([chunk[8], chunk[9], chunk[10], chunk[11]]);
            let p = Vec3::new(x, y, z);
            min = min.min(p);
            max = max.max(p);
            points.extend_from_slice(&[x, y, z]);
        }

        let bbox = BoundingBox { min, max };
        let dim = bbox.dimensions();
        let scale_factor = 1900.0 / dim.y;
        let offset = -(bbox.min + dim * 0.5);

        for chunk in points.chunks_exact_mut(3) {
            let p = Vec3::new(chunk[0], chunk[1], chunk[2]);
            let tp = (p + offset) * scale_factor;
            chunk.copy_from_slice(&tp.to_array());
        }

        let attributes = Self::generate_attributes(points.len() / 3);

        Self {
            points,
            attributes,
            scale: scale_factor,
            center: bbox.center(),
            bbox,
        }
    }

    /// Serialize point cloud back to binary format.
    pub fn to_bytes(&self) -> Vec<u8> {
        let dim = self.bbox.max - self.bbox.min;
        let offset = -(self.bbox.min + dim * 0.5);

        let mut buffer = Vec::with_capacity(self.points.len() / 3 * 12);
        for chunk in self.points.chunks_exact(3) {
            let p = Vec3::new(chunk[0], chunk[1], chunk[2]);
            let original = p / self.scale - offset;
            for v in original.to_array() {
                buffer.extend_from_slice(&v.to_le_bytes());
            }
        }
        buffer
    }

    /// Return the number of points in the cloud.
    pub fn point_count(&self) -> usize {
        self.points.len() / 3
    }

    /// Get the 3D position of a normalized point by index.
    pub fn point(&self, index: usize) -> Option<Vec3> {
        let i = index * 3;
        self.points
            .get(i..i + 3)
            .map(|s| Vec3::new(s[0], s[1], s[2]))
    }

    fn generate_attributes(count: usize) -> Vec<f32> {
        let mut rng = rand::rng();
        (0..count)
            .flat_map(|_| {
                [
                    1.0_f32,                          // active
                    rng.random_range(4.0..8.0),       // size
                    rng.random_range(0..4u32) as f32, // layer
                    rng.random_range(-100.0..100.0),  // delay
                ]
            })
            .collect()
    }
}

impl Default for PointCloud {
    fn default() -> Self {
        Self {
            points: Default::default(),
            attributes: Default::default(),
            scale: 1.0,
            center: Default::default(),
            bbox: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    #[serde(skip)]
    pub data: PointCloud,
    pub offset: Vec3,
    pub pivot: Vec3,
    pub scale: f32,
    pub point_size_scale: f32,
    pub camera_fade_distance: i32,
    pub camera_fade_start: i32,
    pub laser_mode: LaserMode,
}

impl Default for Model {
    fn default() -> Self {
        Self {
            data: Default::default(),
            offset: Default::default(),
            pivot: Default::default(),
            scale: 1.0,
            point_size_scale: 1.0,
            camera_fade_distance: 3500,
            camera_fade_start: 500,
            laser_mode: LaserMode::Ceiling,
        }
    }
}

pub fn load_models() -> Vec<Model> {
    let mut models = Vec::new();

    // Load factory model
    let factory_json = include_str!("../assets/factory.json");
    let mut factory_model: Model = serde_json::from_str(factory_json).unwrap();
    let factory_bin = include_bytes!("../assets/factory.bd9a36.bin");
    factory_model.data = PointCloud::from_bytes(factory_bin);
    models.push(factory_model);

    // Load pile model
    let pile_json = include_str!("../assets/pile.json");
    let mut pile_model: Model = serde_json::from_str(pile_json).unwrap();
    let pile_bin = include_bytes!("../assets/pile.251dc1.bin");
    pile_model.data = PointCloud::from_bytes(pile_bin);
    models.push(pile_model);

    // Load trinity model
    let trinity_json = include_str!("../assets/trinity.json");
    let mut trinity_model: Model = serde_json::from_str(trinity_json).unwrap();
    let trinity_bin = include_bytes!("../assets/trinity.d6c060.bin");
    trinity_model.data = PointCloud::from_bytes(trinity_bin);
    models.push(trinity_model);

    models
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_cloud_from_bytes() {
        let data: Vec<f32> = alloc::vec![
            1.0, 2.0, 3.0, // Point 0
            4.0, 5.0, 6.0, // Point 1
        ];
        let mut buffer = Vec::new();
        for &v in &data {
            buffer.extend_from_slice(&v.to_le_bytes());
        }

        let cloud = PointCloud::from_bytes(&buffer);
        assert_eq!(cloud.point_count(), 2);
        assert_eq!(cloud.point(0).is_some(), true);
        assert_eq!(cloud.point(1).is_some(), true);
    }

    #[test]
    fn test_point_cloud_roundtrip() {
        let original_data: Vec<f32> = alloc::vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0,];
        let mut buffer = Vec::new();
        for &v in &original_data {
            buffer.extend_from_slice(&v.to_le_bytes());
        }

        let cloud = PointCloud::from_bytes(&buffer);
        assert_eq!(cloud.point_count(), 2);

        // Roundtrip: normalized -> binary -> normalized
        let serialized = cloud.to_bytes();
        let restored = PointCloud::from_bytes(&serialized);

        // Points should be approximately equal after roundtrip
        assert_eq!(restored.point_count(), cloud.point_count());
        for i in 0..cloud.point_count() {
            let p1 = cloud.point(i).unwrap();
            let p2 = restored.point(i).unwrap();
            assert!((p1 - p2).length() < 0.1); // Allow small error from normalization
        }
    }
}
