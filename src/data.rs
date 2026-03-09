use std::io::{self, Read};
use std::path::Path;

use glam::Vec3;
use rand::Rng;

#[derive(Debug, Clone, Copy)]
pub struct BoundingBox {
    pub min: Vec3,
    pub max: Vec3,
}

#[derive(Debug, Clone)]
pub struct PointCloud {
    /// Flat array of coordinates: [x0, y0, z0, x1, y1, z1, ...]
    pub points: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct TransformedPointCloud {
    pub points: Vec<f32>,
    pub attributes: Vec<f32>,
    pub scale: f32,
    pub center: Vec3,
    pub bbox: BoundingBox,
}

impl PointCloud {
    /// Load point cloud from binary file.
    pub fn from_file<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let mut file = std::fs::File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        Self::from_bytes(&buffer)
    }

    pub fn from_bytes(buffer: &[u8]) -> io::Result<Self> {
        if buffer.len() % 4 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Buffer length must be multiple of 4 (f32 size)",
            ));
        }

        let mut points = Vec::new();
        for chunk in buffer.chunks_exact(4) {
            let bytes = [chunk[0], chunk[1], chunk[2], chunk[3]];
            let value = f32::from_le_bytes(bytes);
            points.push(value);
        }

        Ok(Self { points })
    }

    /// Serialize point cloud to binary format.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buffer = Vec::new();
        for &value in &self.points {
            buffer.extend_from_slice(&value.to_le_bytes());
        }
        buffer
    }

    pub fn point_count(&self) -> usize {
        self.points.len() / 3
    }

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

    pub fn bounding_box(&self) -> BoundingBox {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);

        for i in (0..self.points.len()).step_by(3) {
            if i + 2 < self.points.len() {
                let p = Vec3::new(self.points[i], self.points[i + 1], self.points[i + 2]);
                min = min.min(p);
                max = max.max(p);
            }
        }

        BoundingBox { min, max }
    }

    /// Normalize and scale to [-0.5*dim, 0.5*dim] range, then scale to 1900 units.
    pub fn transform_normalized(&self) -> TransformedPointCloud {
        let bbox = self.bounding_box();

        let dim = bbox.max - bbox.min;

        // Scale factor based on Y-axis only.
        let scale_factor = 1900.0 / dim.y;

        let offset = Vec3::new(
            -0.5 * dim.x - bbox.min.x,
            -0.5 * dim.y - bbox.min.y,
            -0.5 * dim.z - bbox.min.z,
        );

        // Transform all points.
        let mut transformed = Vec::new();
        for i in (0..self.points.len()).step_by(3) {
            if i + 2 < self.points.len() {
                let p = Vec3::new(self.points[i], self.points[i + 1], self.points[i + 2]);
                let tp = (p + offset) * scale_factor;
                transformed.extend_from_slice(&[tp.x, tp.y, tp.z]);
            }
        }

        // Generate point attributes.
        let point_count = transformed.len() / 3;
        let mut rng = rand::rng();
        let mut attributes = Vec::new();
        for _ in 0..point_count {
            let active = 1.0_f32;
            let size = rng.random_range(4.0..8.0);
            let layer = rng.random_range(1.0..4.0);
            let delay = rng.random_range(-100.0..100.0);
            attributes.extend_from_slice(&[active, size, layer, delay]);
        }

        TransformedPointCloud {
            points: transformed,
            attributes,
            scale: scale_factor,
            center: (bbox.min + bbox.max) * 0.5,
            bbox,
        }
    }

    /// Reverse the normalization transformation.
    pub fn reverse_transform(transformed: &TransformedPointCloud) -> Self {
        let bbox = transformed.bbox;
        let scale_factor = transformed.scale;

        let dim = bbox.max - bbox.min;
        let offset = Vec3::new(
            -0.5 * dim.x - bbox.min.x,
            -0.5 * dim.y - bbox.min.y,
            -0.5 * dim.z - bbox.min.z,
        );

        // Reverse transform: (p / scale) - offset
        let mut points = Vec::new();
        for i in (0..transformed.points.len()).step_by(3) {
            if i + 2 < transformed.points.len() {
                let p = Vec3::new(
                    transformed.points[i],
                    transformed.points[i + 1],
                    transformed.points[i + 2],
                );
                let op = p / scale_factor - offset;
                points.extend_from_slice(&[op.x, op.y, op.z]);
            }
        }

        Self { points }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point_cloud_from_bytes() {
        let data: Vec<f32> = vec![
            1.0, 2.0, 3.0, // Point 0
            4.0, 5.0, 6.0, // Point 1
        ];
        let mut buffer = Vec::new();
        for &v in &data {
            buffer.extend_from_slice(&v.to_le_bytes());
        }

        let cloud = PointCloud::from_bytes(&buffer).unwrap();
        assert_eq!(cloud.point_count(), 2);
        assert_eq!(cloud.point(0), Some(Vec3::new(1.0, 2.0, 3.0)));
        assert_eq!(cloud.point(1), Some(Vec3::new(4.0, 5.0, 6.0)));
    }

    #[test]
    fn test_point_cloud_to_bytes() {
        let cloud = PointCloud {
            points: vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
        };
        let buffer = cloud.to_bytes();
        assert_eq!(buffer.len(), 24); // 6 * 4 bytes

        let restored = PointCloud::from_bytes(&buffer).unwrap();
        assert_eq!(cloud.points, restored.points);
    }

    #[test]
    fn test_bounding_box() {
        let points = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0];
        let cloud = PointCloud { points };
        let bbox = cloud.bounding_box();

        assert_eq!(bbox.min.x, 1.0);
        assert_eq!(bbox.max.x, 7.0);
        assert_eq!(bbox.min.y, 2.0);
        assert_eq!(bbox.max.y, 8.0);
        assert_eq!(bbox.min.z, 3.0);
        assert_eq!(bbox.max.z, 9.0);
    }

    #[test]
    fn test_transform_normalization() {
        let points = vec![0.0, 0.0, 0.0, 100.0, 100.0, 100.0];
        let cloud = PointCloud { points };
        let transformed = cloud.transform_normalized();

        // Should be centered and scaled to 1900 units
        assert!(transformed.scale > 0.0);
        assert_eq!(transformed.center.x, 50.0);
    }

    #[test]
    fn test_transform_reversible() {
        let original_points = vec![10.0, 20.0, 30.0, 40.0, 50.0, 60.0];
        let cloud = PointCloud {
            points: original_points.clone(),
        };

        let transformed = cloud.transform_normalized();
        let restored = PointCloud::reverse_transform(&transformed);

        // Check if points are approximately equal (accounting for floating point errors)
        for i in 0..original_points.len() {
            assert!((original_points[i] - restored.points[i]).abs() < 1e-5);
        }
    }
}
