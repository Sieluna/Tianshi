use alloc::vec::Vec;

use glam::Vec3;
use rand::Rng;
use serde::{Deserialize, Serialize};

use super::error::{Error, Result};

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
    pub fn from_bytes(buffer: &[u8]) -> Result<Self> {
        if buffer.len() % 4 != 0 {
            return Err(Error::InvalidPointCloudLength);
        }

        // Parse raw binary to f32 array
        let mut raw_points = Vec::new();
        for chunk in buffer.chunks_exact(4) {
            let bytes = [chunk[0], chunk[1], chunk[2], chunk[3]];
            let value = f32::from_le_bytes(bytes);
            raw_points.push(value);
        }

        // Compute AABB
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);

        for i in (0..raw_points.len()).step_by(3) {
            if i + 2 < raw_points.len() {
                let p = Vec3::new(raw_points[i], raw_points[i + 1], raw_points[i + 2]);
                min = min.min(p);
                max = max.max(p);
            }
        }

        let bbox = BoundingBox { min, max };
        let dim = bbox.max - bbox.min;

        // Scale factor based on Y-axis only
        let scale_factor = 1900.0 / dim.y;

        let offset = Vec3::new(
            -0.5 * dim.x - bbox.min.x,
            -0.5 * dim.y - bbox.min.y,
            -0.5 * dim.z - bbox.min.z,
        );

        // Transform all points
        let mut points = Vec::new();
        for i in (0..raw_points.len()).step_by(3) {
            if i + 2 < raw_points.len() {
                let p = Vec3::new(raw_points[i], raw_points[i + 1], raw_points[i + 2]);
                let tp: Vec3 = (p + offset) * scale_factor;
                points.extend_from_slice(&[tp.x, tp.y, tp.z]);
            }
        }

        // Generate point attributes
        let point_count = points.len() / 3;
        let mut rng = rand::rng();
        let mut attributes = Vec::new();
        for _ in 0..point_count {
            let active = 1.0_f32;
            let size = rng.random_range(4.0..8.0);
            let layer = rng.random_range(1.0..3.0);
            let delay = rng.random_range(-100.0..100.0);
            attributes.extend_from_slice(&[active, size, layer, delay]);
        }

        Ok(Self {
            points,
            attributes,
            scale: scale_factor,
            center: (bbox.min + bbox.max) * 0.5,
            bbox,
        })
    }

    /// Serialize point cloud back to binary format.
    pub fn to_bytes(&self) -> Vec<u8> {
        let dim = self.bbox.max - self.bbox.min;
        let offset = Vec3::new(
            -0.5 * dim.x - self.bbox.min.x,
            -0.5 * dim.y - self.bbox.min.y,
            -0.5 * dim.z - self.bbox.min.z,
        );

        let mut buffer = Vec::new();
        for i in (0..self.points.len()).step_by(3) {
            if i + 2 < self.points.len() {
                let p = Vec3::new(self.points[i], self.points[i + 1], self.points[i + 2]);
                let op = p / self.scale - offset;
                for &v in &[op.x, op.y, op.z] {
                    buffer.extend_from_slice(&v.to_le_bytes());
                }
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
        if i + 2 < self.points.len() {
            Some(Vec3::new(
                self.points[i],
                self.points[i + 1],
                self.points[i + 2],
            ))
        } else {
            None
        }
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

impl Model {
    /// Get the effective world space offset (offset + pivot).
    pub fn world_offset(&self) -> Vec3 {
        self.offset + self.pivot
    }

    /// Get the effective scale in world space.
    pub fn world_scale(&self) -> f32 {
        self.data.scale * self.scale
    }

    /// Transform a normalized point to world space.
    pub fn world_position(&self, normalized_point: Vec3) -> Vec3 {
        let scale = self.world_scale();
        normalized_point * scale + self.offset + self.pivot * (1.0 - scale)
    }
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
    factory_model.data = PointCloud::from_bytes(factory_bin).unwrap();
    models.push(factory_model);

    // Load pile model
    let pile_json = include_str!("../assets/pile.json");
    let mut pile_model: Model = serde_json::from_str(pile_json).unwrap();
    let pile_bin = include_bytes!("../assets/pile.251dc1.bin");
    pile_model.data = PointCloud::from_bytes(pile_bin).unwrap();
    models.push(pile_model);

    // Load trinity model
    let trinity_json = include_str!("../assets/trinity.json");
    let mut trinity_model: Model = serde_json::from_str(trinity_json).unwrap();
    let trinity_bin = include_bytes!("../assets/trinity.d6c060.bin");
    trinity_model.data = PointCloud::from_bytes(trinity_bin).unwrap();
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

        let cloud = PointCloud::from_bytes(&buffer).unwrap();
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

        let cloud = PointCloud::from_bytes(&buffer).unwrap();
        assert_eq!(cloud.point_count(), 2);

        // Roundtrip: normalized -> binary -> normalized
        let serialized = cloud.to_bytes();
        let restored = PointCloud::from_bytes(&serialized).unwrap();

        // Points should be approximately equal after roundtrip
        assert_eq!(restored.point_count(), cloud.point_count());
        for i in 0..cloud.point_count() {
            let p1 = cloud.point(i).unwrap();
            let p2 = restored.point(i).unwrap();
            assert!((p1 - p2).length() < 0.1); // Allow small error from normalization
        }
    }
}
